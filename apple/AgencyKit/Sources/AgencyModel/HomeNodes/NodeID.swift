import AgencyKit
import AgencyProtocol
import Foundation

/// The id a node puts in its pushes (`agency.node`), worked out for a node the app knows, so that a push is mapped to a node
/// with no extra read: the first 16 bytes of the SHA-256 of `agency-node:` and the node's origin, as 32 lowercase hex
/// characters. The node works out the same (`testdata/node-id.json` pins both). It is not a secret and does not name the host.
public enum NodeID {
    public static func of(_ node: HomeNodeURL) -> String {
        let digest = Sha256.digest(Data("agency-node:\(node.origin)".utf8))
        return digest.prefix(16).map { String(format: "%02x", $0) }.joined()
    }
}
