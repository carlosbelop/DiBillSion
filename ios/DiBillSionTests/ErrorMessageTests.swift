import XCTest
@testable import DiBillSion

final class ErrorMessageTests: XCTestCase {
    func testNotFoundMeansTheInviteOnlyWhenJoining() {
        let notFound = CoreError.Server(code: "not_found", reason: "no such group")
        XCTAssertTrue(AppModel.describe(notFound, joining: true).contains("invite"))
        let syncing = AppModel.describe(notFound)
        XCTAssertFalse(syncing.contains("invite"))
        XCTAssertTrue(syncing.contains("server"))
    }
}
