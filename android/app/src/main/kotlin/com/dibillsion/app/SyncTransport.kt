package com.dibillsion.app

import com.dibillsion.core.HttpRequest
import com.dibillsion.core.HttpResponse
import com.dibillsion.core.HttpTransport
import com.dibillsion.core.TransportException
import java.io.IOException
import java.net.HttpURLConnection
import java.net.URL

/**
 * The raw HTTP the core asks for (ADR 0007): the core builds every request and reads
 * every answer; this only moves bytes with the platform's networking and TLS.
 */
class SyncTransport(private val baseUrl: String) : HttpTransport {
    override fun request(request: HttpRequest): HttpResponse {
        val conn = try {
            URL(baseUrl.trimEnd('/') + request.path).openConnection() as HttpURLConnection
        } catch (e: IOException) {
            throw TransportException.Offline(e.toString())
        }
        try {
            conn.requestMethod = request.method
            conn.connectTimeout = TIMEOUT_MS
            conn.readTimeout = TIMEOUT_MS
            conn.setRequestProperty("Accept", "application/json")
            request.token?.let { conn.setRequestProperty("Authorization", "Bearer $it") }
            request.body?.let { body ->
                conn.doOutput = true
                conn.setRequestProperty("Content-Type", "application/json")
                conn.outputStream.use { it.write(body.toByteArray()) }
            }
            val status = conn.responseCode
            val stream = if (status >= 400) conn.errorStream else conn.inputStream
            val body = stream?.bufferedReader()?.use { it.readText() }.orEmpty()
            return HttpResponse(status.toUShort(), body)
        } catch (e: IOException) {
            throw TransportException.Offline(e.toString())
        } finally {
            conn.disconnect()
        }
    }

    private companion object {
        const val TIMEOUT_MS = 10_000
    }
}
