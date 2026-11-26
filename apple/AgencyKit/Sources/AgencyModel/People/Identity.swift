import Foundation

/// How a participant is drawn: a stable color and the letters inside the face.
public enum Identity {
    public static let hueCount = 8

    /// A stable hue number (1 to 8) from a name, so a participant keeps their color as the team changes.
    public static func hue(of name: String) -> Int {
        var hash: UInt32 = 0
        for scalar in name.unicodeScalars { hash = hash &* 31 &+ scalar.value }
        return Int(hash % UInt32(hueCount)) + 1
    }

    /// One or two characters for inside a face: Build1 → B1, TechLead → TL, Alice → Al, 花子 → 花.
    public static func initials(of label: String) -> String {
        guard let first = label.unicodeScalars.first else { return "" }
        if first.value > 0x7F { return String(first) }
        if let numbered = numbered(label) { return numbered }
        let capitals = label.filter { $0.isASCII && $0.isUppercase }
        if capitals.count >= 2 { return String(capitals.prefix(2)) }
        return label.prefix(1).uppercased() + label.dropFirst().prefix(1)
    }

    /// A letter, more letters, and digits to the end: the letter and the digits.
    private static func numbered(_ label: String) -> String? {
        let characters = Array(label)
        guard let letter = characters.first, letter.isASCII, letter.isLetter else { return nil }
        let digitsStart = characters.lastIndex { !($0.isASCII && $0.isNumber) }.map { $0 + 1 } ?? 0
        guard digitsStart < characters.count, digitsStart >= 1 else { return nil }
        let middle = characters[1..<digitsStart]
        guard middle.allSatisfy({ $0.isASCII && $0.isLetter }) else { return nil }
        return letter.uppercased() + String(characters[digitsStart...])
    }
}
