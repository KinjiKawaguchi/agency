import AgencyAPI
import AgencyKit
import Foundation

/// Sends the Client API's requests to the home node. It never follows a redirect: Access answers an
/// ended session with one, and the client reads the 3xx as the end of the session. Nothing is cached or
/// kept in cookies.
final class URLSessionTransport: NSObject, HTTPTransport, URLSessionTaskDelegate, @unchecked Sendable {
    private let base: URL
    private var session: URLSession!

    init(homeNode: HomeNodeURL) {
        base = homeNode.url
        super.init()
        let configuration = URLSessionConfiguration.ephemeral
        configuration.httpShouldSetCookies = false
        configuration.httpCookieAcceptPolicy = .never
        configuration.waitsForConnectivity = false
        session = URLSession(configuration: configuration, delegate: self, delegateQueue: nil)
    }

    func send(_ request: HTTPRequest) async throws -> HTTPResponse {
        guard request.path.hasPrefix("/"), !request.path.hasPrefix("//"),
              let url = URL(string: request.path, relativeTo: base)?.absoluteURL,
              url.host == base.host, url.scheme == base.scheme
        else { throw NodeError.unreadable("not a path of the home node: \(request.path)") }
        var urlRequest = URLRequest(url: url, cachePolicy: .reloadIgnoringLocalCacheData, timeoutInterval: 30)
        urlRequest.httpMethod = request.method
        urlRequest.httpBody = request.body
        urlRequest.setValue("application/json", forHTTPHeaderField: "accept")
        for (name, value) in request.headers { urlRequest.setValue(value, forHTTPHeaderField: name) }
        let (data, response) = try await session.data(for: urlRequest)
        guard let http = response as? HTTPURLResponse else { throw NodeError.unreadable("not an HTTP answer") }
        return HTTPResponse(status: http.statusCode, body: data)
    }

    func urlSession(
        _ session: URLSession, task: URLSessionTask, willPerformHTTPRedirection response: HTTPURLResponse,
        newRequest request: URLRequest, completionHandler: @escaping (URLRequest?) -> Void
    ) {
        completionHandler(nil)
    }
}
