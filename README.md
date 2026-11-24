# Agency

AI エージェントと人が一緒に働く組織を、複数の機械にまたがって動かすためのノード。

- エージェントのチームは、それぞれの機械 (自分の Mac、研究室のサーバー、職場の PC) のハーネスで動く。
- 人は自分の台から、所属するすべての組織の「決めること」(問い、稟議、判断待ち) に答える。
- 答えは決めた人の鍵で署名した封筒になり、チームの台に届く。相手の台が寝ていれば、ため置いて、起きたら届く。

中央のサーバーは置かない。
中央にあるのは権限 (組織の根の鍵) だけで、データは機械ごとに分かれ、どのデータも書き手は 1 つ。
約束の全体は [docs/protocol.md](docs/protocol.md)。

## 単位

| 種類 | 単位 | 書き手 |
|---|---|---|
| 実行 (稼働、要求、決裁、ボード) | チーム (ハーネス 1 つ) | そのハーネスが動いている台 |
| 統治 (部署、人、チームの置き場所、取り消した鍵) | 組織 | 組織の根の鍵 |
| 人 (鍵、所属) | アカウント | その人 |

## 中身

```
crates/agency-proto   約束: 決まった順の JSON、Ed25519 の署名、封筒、組織の定義の版、招待と参加
crates/agency-node    ノード (コマンド agency): 台どうしの道、写し、送り箱、ハーネスとの境目
docs/protocol.md      約束の文書
testdata/vectors.json 言語に依らない試験の値 (決まった種の鍵、決まった順の JSON、署名)
```

## 作る、試す

```sh
cargo build --release
cargo test
cargo clippy --all-targets
```

## 使い方

組織を作り、チームを置き、人を招く (根の鍵の台で):

```sh
agency --dir ~/.agency init --account alice --label Alice --url https://alice.example --scrum /path/to/scrum
agency --dir ~/.agency org-create --org acme --label Acme
agency --dir ~/.agency team-add --org acme --team app --label app --state-dir ~/.claude/scrum
agency --dir ~/.agency invite --org acme --role member
agency --dir ~/.agency serve
```

招待のリンクから参加し、答える (招かれた人の台で):

```sh
agency --dir ~/.agency init --account bob --label Bob --url https://bob.example
agency --dir ~/.agency join 'agency:join?node=...&org=acme&fp=...&code=...'
agency --dir ~/.agency sync
agency --dir ~/.agency decide --org acme --team app --request r-20260101-000000-abcd --choice ア
```

招待のリンクには組織の根の鍵の指紋が入っている。
参加する前に、台が返した組織の定義の鍵がその指紋と合うかを確かめ、合わなければ申し込まない。

## Team runtimes

The node never reads or writes a harness's files.
For each team it hosts, it runs the team's runtime command and speaks agency-runtime/1 (`describe`, `view`, `decide`) as JSON over stdin and stdout; see docs/protocol.md.
`team-add --state-dir` configures the scrum harness's runtime, `<scrum> runtime` with `SCRUM_STATE_DIR`, owned by this node's account unless `--owner` names another org person.

## まだ無いもの

- 画面 (ノードが配る、決めること、人と仕事、組織図)
- 鍵をブラウザや端末の側に持つこと (今はノードがアカウントの鍵を持つ)
- 部署の長が自分の部署の定義に署名すること (今は組織の根の鍵だけが署名する)
- 鍵をなくしたとき、人が抜けたときの取り消しの流れ

## ライセンス

MIT と Apache-2.0 の、どちらか好きなほう ([LICENSE-MIT](LICENSE-MIT)、[LICENSE-APACHE](LICENSE-APACHE))。
