import Crypto
import Foundation

/// SHA-256, for the parts of the apps that have no cryptography of their own.
public enum Sha256 {
    public static func digest(_ data: Data) -> Data {
        Data(SHA256.hash(data: data))
    }
}
