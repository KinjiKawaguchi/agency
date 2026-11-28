// The notices the app gives of what is new (requests, participants, blocked work, devices, decisions), and where they are sent.
import type { Messages } from '../en'

export const notices: Messages['notices'] = {
  // The words of the iPhone app's banner (apple/AgencyKit/Sources/AgencyModel/Wording/JaNews.swift), so both clients say the same.
  request: (count, title, team) => (count === 1 ? `${team}: 「${title}」があなたを待っています` : `${count} 件のリクエストがあなたを待っています`),
  participant: (count, label) => (count === 1 ? `${label} があなたの返事を待っています` : `メンバー ${count} 人があなたの返事を待っています`),
  blocked: (count, title, team, reason) => (count === 1 ? `${team}: 「${title}」が止まりました${reason === undefined ? '' : ` (${reason})`}` : `${count} 件の仕事が止まりました`),
  aDevice: '端末',
  device: (count, label) => (count === 1 ? `端末「${label}」が追加を待っています` : `${count} 台の端末が追加を待っています`),
  decision: (state, count, title, reason) => {
    if (count > 1) return `${count} 件の決定に変化がありました`
    const named = title === '' ? '決定' : `「${title}」の決定`
    if (state === 'refused') return `${named}は断られました${reason === undefined ? '' : `: ${reason}`}`
    if (state === 'queued') return `${named}は保存済みですが、まだ届いていません`
    return `${named}が${state === 'applied' ? '反映されました' : '届きました'}`
  },
  more: (count) => `ほかにも新しいことが ${count} 件あります。`,
  // The iPhone's words for a push whose titles are hidden (apple/Shared/ja.lproj), and the same for the kinds it has no push for.
  hidden: {
    request: 'あなたを待っているリクエストがあります',
    participant: 'メンバーがあなたの返事を待っています',
    blocked: '止まった仕事があります',
    device: '端末が追加を待っています',
    decisionRefused: '決定が断られました',
  },
  devicesMark: (count) => `承認待ち ${count} 件`,
  destinations: {
    title: '通知の送信先',
    lead: 'このノードが通知を送るブラウザとアプリの一覧です。端末を削除すると、その端末への通知は自動で止まります。どの端末にも紐づかない登録は、ここで止めるか、そのブラウザやアプリで通知をオフにするまで残ります。',
    thisBrowser: 'このブラウザ',
    turnOffHere: 'このブラウザの通知をオフにする',
    loading: 'ノードに問い合わせています…',
    failed: (reason) => `通知の送信先を読めませんでした: ${reason}`,
    none: 'このノードは、どこにも通知を送っていません。',
    kinds: { browser: 'ブラウザ', app: 'iPhone / Mac のアプリ' },
    registered: (when) => `${when}に登録`,
    tiedTo: (label) => `${label} に紐づいています。`,
    tiedToNone: 'どの端末にも紐づいていません。止めるまで残ります。',
    tiedToGone: (id) => `もうあなたのものではない端末(${id})に紐づいています。`,
    stopFor: (label) => `${label}への通知を止める`,
    stop: '通知を止める',
    confirmTitle: (label) => `${label}への通知を止めますか?`,
    confirmBody: (label, when) => `${label}(${when}に登録)は、このノードからの通知を受け取らなくなります。そのブラウザやアプリで通知がオンのまま Agency をまた開くと、自分でまた登録します。止め続けるには、そちらで通知をオフにするか、この画面でその端末を削除してください。`,
    confirm: '通知を止める',
    cancel: 'キャンセル',
    stopping: '止めています…',
    stopped: (label) => `${label}への通知を止めました。`,
    stopFailed: (reason) => `通知を止められませんでした: ${reason}`,
  },
  os: {
    title: 'このブラウザの通知',
    lead: 'このページが隠れているあいだ、新しいことをオペレーティングシステムの通知で知らせます。言葉はページの通知と同じで、このブラウザの外には出ません。',
    turnOn: '通知を有効にする',
    turnOff: '通知を無効にする',
    isOn: '有効です。このページが隠れているときに新しいことがあると、通知が出ます。気づくために、ブラウザが許す限りページは背面でも読み込みを続けますが、ブラウザを閉じているあいだは読み込めません。',
    isBlocked: 'このブラウザは、このページの通知を許可していません。サイトの設定で許可してから、ここで有効にしてください。',
    isUnsupported: 'このブラウザは、ページからの通知を出せません。',
    push: {
      title: 'アプリを閉じているとき',
      lead: '新しいことを、ノードがこのブラウザのプッシュサービスを通して知らせます。ページを開いていなくても届きます。文言はノードのものです。タイトルを隠すノードは、タイトルを送りません。',
      turnOn: '閉じているときも通知する',
      turnOff: '閉じているときの通知をやめる',
      isOn: '有効です。ページを閉じていても、新しいことがあるとノードがこのブラウザに知らせます。',
      isFailing: (when, reason) => `このブラウザのプッシュサービスは、最後の通知を受け付けませんでした（${when}）: ${reason}。ノードは最大 1 日のあいだ再送します。失敗が続くときは、いったんやめて、もう一度有効にしてください。`,
      lastTaken: (when) => `最後に受け付けた通知は ${when} のものです。`,
      neverTaken: 'まだ 1 件も受け付けていません。',
      isStale: 'このブラウザは、ノードがもう持っていない鍵で登録されているため、通知が届きません。登録し直してください。',
      subscribeAgain: '登録し直す',
      failed: (message) => `変更できませんでした: ${message}`,
    },
  },
}
