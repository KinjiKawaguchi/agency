// The overview screen.
import type { Messages } from '../en'

export const overview: Messages['overview'] = {
  title: '全体の状況',
  lead: (requests, teams) => (requests === 0 ? 'あなたを待っている依頼は、どのチームにもありません。' : `${teams} チームで、${requests} 件の依頼があなたを待っています。`),
  leadPartial: (requests, teams, unread) => `${requests === 0 ? '答えたチームでは、あなたを待っている依頼はありません。' : `答えた ${teams} チームで、${requests} 件の依頼があなたを待っています。`}読み込めなかったチームが ${unread} あります。`,
  noTeams: 'この組織には、まだチームがありません。',
  waitingForYou: (count) => `あなたを待っている依頼 ${count} 件`,
  nothingWaits: 'あなたを待っている依頼はありません。',
  working: (working, total) => `参加者 ${total} 人のうち ${working} 人が作業中`,
  noParticipants: '参加者の報告がありません。',
  tags: { mine: 'あなたの番', stuck: '止まっている', idle: '動いていない', failed: '読み込めません' },
  blocked: (count) => `止まっている項目 ${count} 件`,
  nobodyWorking: '作業中の参加者はいません。',
  usage: (amount) => `今週 ${amount} (API 換算)`,
  usageNone: 'このチームはトークンの使用量を報告していません。',
  usageFailed: 'トークンの使用量を読めませんでした。',
  dataFrom: (relative) => `${relative} のデータ`,
  staleFrom: (relative, reason) => `${relative} のデータです。チームのノードに接続できません: ${reason}`,
  neverRead: (reason) => `まだ読めていません: ${reason}`,
  loadFailed: (reason) => `このチームを読み込めませんでした: ${reason}`,
  funding: '利用上限',
}
