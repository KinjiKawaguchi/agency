@testable import AgencyModel
import Testing

@Suite("Menu bar")
struct MenuBarWordingTests {
    @Test("the state tells no home node, not read, read failed, and the last number apart")
    func state() {
        #expect(MenuBarState(hasHomeNode: false, known: nil, isFailed: false) == .noHomeNode)
        #expect(MenuBarState(hasHomeNode: false, known: 3, isFailed: true) == .noHomeNode, "with no home node a number is not the node's")
        #expect(MenuBarState(hasHomeNode: true, known: nil, isFailed: false) == .notRead)
        #expect(MenuBarState(hasHomeNode: true, known: nil, isFailed: true) == .readFailed)
        #expect(MenuBarState(hasHomeNode: true, known: 0, isFailed: false) == .count(0, isStale: false))
        #expect(MenuBarState(hasHomeNode: true, known: 4, isFailed: false) == .count(4, isStale: false))
        #expect(MenuBarState(hasHomeNode: true, known: 4, isFailed: true) == .count(4, isStale: true), "a failed read leaves the last number, marked")
    }

    @Test("the label shows a number only when something waits, stale or not")
    func label() {
        #expect(MenuBarState.noHomeNode.label == nil)
        #expect(MenuBarState.notRead.label == nil)
        #expect(MenuBarState.readFailed.label == nil)
        #expect(MenuBarState.count(0, isStale: false).label == nil)
        #expect(MenuBarState.count(3, isStale: false).label == "3")
        #expect(MenuBarState.count(3, isStale: true).label == "3")
    }

    @Test("the first line says what is the case, and 0 is not 'not read'")
    func status() {
        #expect(Ja.MenuBar.status(.noHomeNode) == "ホームノードが設定されていません")
        #expect(Ja.MenuBar.status(.notRead) == "まだ読み込んでいません")
        #expect(Ja.MenuBar.status(.readFailed) == "読み込めませんでした")
        #expect(Ja.MenuBar.status(.count(0, isStale: false)) == "あなたを待っている依頼はありません")
        #expect(Ja.MenuBar.status(.count(2, isStale: false)) == "2 件があなたを待っています")
        #expect(Ja.MenuBar.status(.count(2, isStale: true)) == "2 件があなたを待っています")
    }

    @Test("a number that is not the latest is dim and says so; nothing else is dim except a failed first read")
    func dimAndNote() {
        #expect(Ja.MenuBar.isDim(.count(2, isStale: true)))
        #expect(!Ja.MenuBar.isDim(.count(2, isStale: false)))
        #expect(Ja.MenuBar.isDim(.readFailed))
        #expect(!Ja.MenuBar.isDim(.noHomeNode) && !Ja.MenuBar.isDim(.notRead))
        #expect(Ja.MenuBar.note(.count(2, isStale: true)) == "最新の数を読み込めませんでした。前に読み込んだときの数です")
        #expect(Ja.MenuBar.note(.count(2, isStale: false)) == nil)
        #expect(Ja.MenuBar.note(.readFailed) == nil, "its first line already says it")
    }

    @Test("a screen reader hears the app's name, the first line, and the note when there is one")
    func accessibility() {
        #expect(Ja.MenuBar.accessibility(.notRead) == "Agency: まだ読み込んでいません")
        #expect(Ja.MenuBar.accessibility(.noHomeNode) == "Agency: ホームノードが設定されていません")
        #expect(Ja.MenuBar.accessibility(.count(4, isStale: false)) == "Agency: 4 件があなたを待っています")
        #expect(Ja.MenuBar.accessibility(.count(4, isStale: true)) == "Agency: 4 件があなたを待っています。最新の数を読み込めませんでした。前に読み込んだときの数です")
    }
}

private extension MenuBarState {
    var label: String? { Ja.MenuBar.label(self) }
}
