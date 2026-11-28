// The frame of every screen: the app’s own messages, the menu, the settings, the person, the time, and what shows while a page loads.
import type { Messages } from '../en'

export const app: Messages['app'] = {
  name: 'Agency',
  loading: '読み込み中…',
  replica: {
    staleCopy: (when, reason) => `このチームは ${when} に読んだ写しを表示しています。チームのノードに接続できません: ${reason}`,
    neverRead: (reason) => `このチームのノードはまだ読めていません: ${reason}`,
  },
  unreachable: (reason) => `ノードに接続できません: ${reason}`,
  signInExpired: { message: 'サインインの有効期限が切れました。再読み込みして、もう一度サインインしてください。', reload: '再読み込み', nothingToShow: 'サインインし直すまで、表示できるものがありません。' },
  pageLoad: {
    failed: 'このページを読み込めませんでした。',
    tryAgain: 'もう一度試す',
    newVersion: 'このページを読み込めませんでした。新しい版の Agency が入った可能性があり、このタブが持っている画面は古くなっています。',
    reload: '再読み込み',
  },
}

export const nav: Messages['nav'] = {
  label: 'メインメニュー',
  desk: 'あなた宛てのリクエスト',
  overview: 'チームの状況',
  people: 'メンバーの作業状況',
  org: '組織図',
  organization: '組織',
  results: '成果',
  section: (group) => `${group}のページ`,
  costs: 'コスト',
  evaluation: '評価',
  live: 'ライブ',
  outcomes: 'KPI',
  devices: '端末',
  members: '組織のメンバー',
  history: '組織の変更履歴',
}

export const settings: Messages['settings'] = {
  theme: '配色',
  themeAuto: '自動',
  themeLight: '明るい',
  themeDark: '暗い',
  language: '言語',
}

export const person: Messages['person'] = {
  states: {
    working: '作業中',
    'needs-you': 'あなたを待っています',
    waiting: '誰かを待っています',
    idle: '待機中',
    away: '席を外しています',
  },
}

export const time: Messages['time'] = {
  justNow: 'たった今',
}

export const loading: Messages['loading'] = {
  label: '読み込んでいます…',
  failed: (what, reason) => `${what}を読み込めませんでした: ${reason}`,
  retryIn: (seconds) => `${seconds} 秒後にもう一度試します。`,
  retrying: 'もう一度試しています…',
  tryAgain: 'もう一度試す',
  what: { overview: 'チームの状況', orgChart: '組織図', people: 'メンバーの作業状況' },
}
