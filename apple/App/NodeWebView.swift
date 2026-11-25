import SwiftUI
import WebKit

struct NodeWebView: UIViewRepresentable {
    let model: NodeWebModel

    func makeUIView(context: Context) -> WKWebView { model.webView }

    func updateUIView(_ webView: WKWebView, context: Context) {}
}
