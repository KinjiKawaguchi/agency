// The live page: what the agents are doing now.
import type { Messages } from '../en'

export const live: Messages['live'] = {
  title: 'ライブ',
  experimental: '実験中',
  lead: '開いたままにしておくページです。何が追加されているか、誰が誰に書いているか、各 AI エージェントが何をしているか、何が入ったかを見せます。まだ粗いままです。',
  notReported: 'このノードはまだアクティビティを返しません。誰が何をしているかはこれまでどおりチームから出し、フィードとメッセージはノードが返すようになってから始まります。',
  delayed: (teams) => `${teams}のフィードが遅れています。`,
  noAgents: 'まだ描く AI エージェントがいません。',
  status: { backlog: 'バックログ', next: '次', inProgress: '進行中', release: 'リリース', done: '完了' },
  graph: {
    title: '誰が誰に書いているか',
    hint: 'メッセージが通ると線が光ります。やり取りの多い線ほど太くなります。',
    label: 'AI エージェント間のメッセージ: AI エージェントを点、やり取りを線で表した図',
    agent: (name, team) => `${name} (${team}チーム)`,
    others: 'ほか (ここのチームにいない)',
    othersMore: (shown, total) => `ほか (ここのチームにいない): ${total} 人のうち ${shown} 人を表示`,
  },
  feed: {
    title: '今起きていること',
    empty: 'この 1 時間はまだ何もありません。',
    notRead: 'まだフィードを読み込めていません。',
    added: (ref, title) => `新規: ${ref} ${title}`,
    changed: (ref, title, from, to) => `${ref} ${title}: ${from} → ${to}`,
    opened: (ref, title) => `プルリクエストを開きました: ${ref} ${title}`,
    merged: (ref, title) => `プルリクエストをマージしました: ${ref} ${title}`,
    leadTime: (duration) => `${duration}で`,
    message: (from, to) => `${from} → ${to}`,
  },
  board: {
    title: '今の仕事',
    idle: '割り当てなし',
    more: (count) => `ほか ${count} 件`,
    epic: (ref, title) => `${title} (${ref}) の中`,
  },
  throughput: {
    title: 'スループット',
    lastHour: '直近 1 時間にマージした数',
    today: '今日マージした数',
    lead: 'リードタイム (プルリクエスト、中央値)',
    none: '—',
    bins: '直近 1 時間の 5 分ごとのマージ数',
    landed: (ref) => `入りました: ${ref}`,
  },
}
