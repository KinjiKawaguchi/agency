// The overview screen.
import type { Messages } from '../en'

export const overview: Messages['overview'] = {
  title: 'チームの状況',
  lead: (requests, teams) => (requests === 0 ? 'あなたを待っているリクエストは、どのチームにもありません。' : `${teams} チームで、${requests} 件のリクエストがあなたを待っています。`),
  leadPartial: (requests, teams, unread) => `${requests === 0 ? '答えたチームでは、あなたを待っているリクエストはありません。' : `答えた ${teams} チームで、${requests} 件のリクエストがあなたを待っています。`}読み込めなかったチームが ${unread} あります。`,
  noTeams: 'この組織には、まだチームがありません。',
  waitingForYou: (count) => `あなたを待っているリクエスト ${count} 件`,
  nothingWaits: 'あなたを待っているリクエストはありません。',
  working: (working, total) => `メンバー ${total} 人のうち ${working} 人が作業中`,
  noParticipants: 'メンバーの報告がありません。',
  tags: { mine: 'あなたの番', stuck: '止まっている', idle: '動いていない', failed: '読み込めません' },
  blocked: (count) => `止まっている項目 ${count} 件`,
  nobodyWorking: '作業中のメンバーはいません。',
  usage: (amount) => `今週 ${amount} (API 換算)`,
  usageNone: 'このチームはトークンの使用量を報告していません。',
  usageFailed: 'トークンの使用量を読めませんでした。',
  dataFrom: (relative) => `${relative} のデータ`,
  staleFrom: (relative, reason) => `${relative} のデータです。チームのノードに接続できません: ${reason}`,
  neverRead: (reason) => `まだ読めていません: ${reason}`,
  loadFailed: (reason) => `このチームを読み込めませんでした: ${reason}`,
  funding: '利用上限',
}
