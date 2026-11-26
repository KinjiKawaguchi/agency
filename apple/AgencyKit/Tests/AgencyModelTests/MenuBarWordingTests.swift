@testable import AgencyModel
import Testing

@Suite("Menu bar wording")
struct MenuBarWordingTests {
    @Test("the label shows a number only when something waits")
    func label() {
        #expect(Ja.MenuBar.label(nil) == nil)
        #expect(Ja.MenuBar.label(0) == nil)
        #expect(Ja.MenuBar.label(3) == "3")
    }

    @Test("the first line says not read, nothing waiting, or how many")
    func status() {
        #expect(Ja.MenuBar.status(nil) == "まだ読み込んでいません")
        #expect(Ja.MenuBar.status(0) == "あなたを待っている依頼はありません")
        #expect(Ja.MenuBar.status(2) == "2 件があなたを待っています")
    }
}
