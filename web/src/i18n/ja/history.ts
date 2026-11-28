// The change history screen.
import type { Messages } from '../en'

export const history: Messages['history'] = {
  title: '履歴',
  lead: 'ノードが保管している組織の版を、古い順に見ます。ルートがいつ署名したか、署名が何を示したか、何を変えたかです。読むだけで、署名の判定は、この画面ではなくノードのものです。',
  org: '組織',
  noOrgs: 'どの組織にも入っていません。',
  loading: 'ノードに問い合わせています…',
  unavailable: 'このノードは、履歴をまだ返しません。',
  empty: 'このノードは、この組織の版を保管していません。',
  version: (number) => `版 ${number}`,
  signed: (when) => `${when}に署名`,
  signedBy: (key) => `署名した鍵: ${key}`,
  signatures: { valid: '署名は正しいです', invalid: '署名は正しくありません: 署名のあとに版が変えられたか、組織のルートが署名していません', notChecked: '署名は確かめていません: ひとつ前の版が、このノードにありません' },
  notKept: 'このノードは、この版を保管していません。',
  fork: 'ひとつ前の版に続いていません: 履歴がここで分かれています。',
  comparedWith: (number) => `間の版がこのノードにないため、変更は版 ${number} からの分です。`,
  noChanges: '変更は書かれていません。',
  more: '後の版を見る',
  loadingMore: 'ノードに問い合わせています…',
  moreFailed: (reason) => `後の版を読めませんでした: ${reason}`,
  retry: 'もう一度試す',
  source: (path) => `エージェントが読む同じ項目: GET ${path}。`,
}
