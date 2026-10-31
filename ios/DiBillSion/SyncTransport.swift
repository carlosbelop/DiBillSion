import Foundation
#if canImport(FoundationNetworking)
import FoundationNetworking
#endif

/// The raw HTTP the core asks for (ADR 0007): the core builds every request and reads
/// every answer; this only moves bytes with `URLSession` and the system's TLS.
/// The core calls it from the background thread it runs on, so blocking here is fine.
final class SyncTransport: HttpTransport, @unchecked Sendable {
    private let baseURL: String
    private let session: URLSession

    init(baseURL: String) {
        self.baseURL = baseURL.hasSuffix("/") ? String(baseURL.dropLast()) : baseURL
        let config = URLSessionConfiguration.ephemeral
        config.timeoutIntervalForRequest = 10
        session = URLSession(configuration: config)
    }

    /// The server configured for this build (Info.plist `DibillSyncURL`), if any.
    static func fromBundle(_ bundle: Bundle = .main) -> SyncTransport? {
        guard let url = bundle.object(forInfoDictionaryKey: "DibillSyncURL") as? String,
              !url.isEmpty else { return nil }
        return SyncTransport(baseURL: url)
    }

    func request(request: HttpRequest) throws -> HttpResponse {
        guard let url = URL(string: baseURL + request.path) else {
            throw TransportError.Offline(reason: "bad URL: \(baseURL)\(request.path)")
        }
        var req = URLRequest(url: url)
        req.httpMethod = request.method
        req.setValue("application/json", forHTTPHeaderField: "Accept")
        if let token = request.token {
            req.setValue("Bearer \(token)", forHTTPHeaderField: "Authorization")
        }
        if let body = request.body {
            req.setValue("application/json", forHTTPHeaderField: "Content-Type")
            req.httpBody = Data(body.utf8)
        }

        let done = DispatchSemaphore(value: 0)
        nonisolated(unsafe) var result: Result<HttpResponse, TransportError> =
            .failure(.Offline(reason: "no answer"))
        let task = session.dataTask(with: req) { data, response, error in
            defer { done.signal() }
            if let error {
                result = .failure(.Offline(reason: error.localizedDescription))
            } else if let http = response as? HTTPURLResponse {
                let body = data.flatMap { String(data: $0, encoding: .utf8) } ?? ""
                result = .success(HttpResponse(status: UInt16(clamping: http.statusCode), body: body))
            }
        }
        task.resume()
        done.wait()
        return try result.get()
    }
}
