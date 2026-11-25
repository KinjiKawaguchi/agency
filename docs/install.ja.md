# ノードのインストール

この文書では、Agency のノードを Mac にソースからインストールし、launchd のエージェントとして動かし、最初の端末を追加するまでを説明します。
続いて、そのノードを Cloudflare Access の背後に置き、ほかのマシンから使えるようにする方法を説明します。
いまのところ対象は macOS だけで、Linux と systemd での手順は今後追加します。

英語版は [install.md](install.md) で、節の構成は同じです。
内容が食い違うときは英語版が正です。

例ではアカウント `alice`、組織 `acme`、チーム `app`、ホスト名 `agency.example.com` を使います。
実際には自分の名前に置き換えてください。
ID に使えるのは英小文字、数字、ハイフンです（`[a-z0-9][a-z0-9-]{0,39}`）。

## 必要なもの

- macOS（Apple シリコンでも Intel でもかまいません）。
- Xcode Command Line Tools。`xcode-select --install` で入ります。
- Rust の stable 版、1.87 以降。[rustup](https://rustup.rs) で入れ、`rustc --version` で確かめます。
- Web クライアントのビルドに使う Node.js 24 と npm。`node --version` で確かめます。
- チームのハーネスと、ハーネスが使うツール（[チームランタイム](#チームランタイム)を参照）。

リポジトリ、データディレクトリ、チームの状態ディレクトリは、`~/Documents`、`~/Desktop`、`~/Downloads` の外に置いてください。
macOS は、フルディスクアクセスを与えていない launchd のエージェントにこれらのフォルダを読ませません。

## ソースからのビルド

先に Web クライアントをビルドします。
ノードはビルド時に Web クライアントを取り込むためです。

```sh
git clone <このリポジトリの URL> ~/src/agency
cd ~/src/agency/web
npm ci
npm run build
cd ..
cargo build --release -p agency-node
```

できたバイナリは、再ビルドや `cargo clean` の影響を受けない場所に置きます。
いったん別名でコピーしてから名前を変えるのは、動いているノードのバイナリをその場で書き換えないためです。

```sh
mkdir -p ~/.local/bin
cp target/release/agency ~/.local/bin/agency.new && mv ~/.local/bin/agency.new ~/.local/bin/agency
~/.local/bin/agency --help
```

以下ではこれを `agency` と書きます。
`~/.local/bin` を `PATH` に入れるか、フルパスで実行してください。
`agency` は `--dir` を指定しないと `$AGENCY_DIR` に、それも未設定なら `~/.agency` にデータを置きます。

## ローカル専用のノード

ローカル専用のノードは、1 台の Mac の中だけで動きます。
待ち受けるのは `127.0.0.1` だけなので、Web クライアントはその Mac でしか開けず、ノードもその Mac からのリクエストしか受け付けません。
ほかのノードとは行き来がなく、トンネルも Cloudflare もプッシュ通知も使いません。
ノードが保存するものはすべてデータディレクトリの中に残ります。

### ノード、組織、チームを作る

```sh
agency init --account alice --label "Alice" \
  --url http://127.0.0.1:4400 --listen 127.0.0.1:4400 \
  --scrum "$(command -v scrum)"
agency org-create --org acme --label "Acme"
agency team-add --org acme --team app --label "App" --state-dir ~/teams/app
agency status
```

- `--url` は、ほかのノードがこのノードに届くときのアドレスです。ローカル専用のノードでは自分のループバックアドレスを指定し、ポートは `--listen` とそろえます。
- `--scrum` はハーネスのコマンドです。上の例のようにフルパスで指定するか、ノードが自分の `PATH` から探すコマンド名だけを指定します。
- `--state-dir` は、ハーネスがチームの状態を置くディレクトリです。相対パスを渡しても絶対パスで保存されます。
- `org-create` は、このノードのアカウントを組織のルート鍵にします。`--node` を付けない `team-add` は、このノードのアカウントを持ち主として、チームをこのノードで動かします。

`agency status` に、アカウント、`Org acme (Acme), version 2, root key`、`Team on this node: acme/app (owner alice) runs <scrum> runtime` が表示されれば成功です。

データディレクトリには、組織のルート鍵である `account.key` ができています。
このファイルを読める人は、組織のルートとして振る舞えます。
この Mac の外に持ち出さず、同期や共有をするフォルダにも置かないでください。

### launchd で動かす

エージェントの定義ファイルは `scripts/launchd-plist.sh` で作ります。
エージェントにはスクリプトを実行したシェルの `PATH` が渡るので、チームランタイムはターミナルと同じツールを見つけられます。
`scrum` とハーネスのツールが動くターミナルで実行してください。

```sh
scripts/launchd-plist.sh --agency ~/.local/bin/agency --dir ~/.agency \
  --out ~/Library/LaunchAgents/local.agency.node.plist
plutil -lint ~/Library/LaunchAgents/local.agency.node.plist
launchctl bootstrap gui/$(id -u) ~/Library/LaunchAgents/local.agency.node.plist
```

エージェントはログイン時に `agency serve` を起動し、止まったら起動し直します。
ノードの出力は `~/.agency/logs/serve.log` に書かれます。

| したいこと | コマンド |
|---|---|
| 動いているか確かめる | `launchctl print gui/$(id -u)/local.agency.node \| grep state` |
| 再起動する | `launchctl kickstart -k gui/$(id -u)/local.agency.node` |
| 次に読み込むまで止める | `launchctl bootout gui/$(id -u)/local.agency.node` |

ハーネスの置き場所を変えたときなど、`PATH` が変わったら、スクリプトを実行し直してから `bootout` と `bootstrap` をやり直します。

### 最初の端末としてブラウザを追加する

決定に署名するのは自分の端末で、ノードは署名しません。
最初の端末だけは、ノードのターミナルでノードのアカウント鍵を使って追加します。

1. ブラウザで `http://127.0.0.1:4400/` を開きます。アドレスは常にこれを使ってください。ブラウザにとって `http://localhost:4400/` は別のサイトなので、そちらには別の鍵ができてしまいます。
2. 「端末」を開き、「この端末を追加」を選びます。`8945-bea8-e6d8` のような照合コードが表示されます。
3. ターミナルで `agency device-add` を実行します。`Add "<名前>" with pairing code 8945-bea8-e6d8?` と聞かれるので、ブラウザのコードと 1 文字残らず一致するときだけ `y` と答えます。
4. ブラウザの画面に、端末を追加したと表示されます。

ターミナルからも決定したい（`agency decide` を使う）ときは、`agency device-add --cli` で CLI の端末を追加します。
ブラウザを追加したあとでこれを実行すると、CLI の端末は追加の依頼として登録されます。
ブラウザの「端末」でコードを見比べてから承認してください。

### 動作を確かめる

```sh
agency status
curl -s http://127.0.0.1:4400/api/me
curl -s -w ' %{http_code}\n' http://127.0.0.1:4400/api/orgs/acme/teams/app/view | tail -c 200
tail -n 20 ~/.agency/logs/serve.log
```

- `/api/me` には、アカウントと、チーム `app` を含む組織が `"location":"local"` 付きで出ます。
- チームのビューの末尾が `200` なら正常です。`503` のときは、応答の `error` にノードがチームランタイムを動かせなかった理由が書かれています（[トラブルシューティング](#トラブルシューティング)を参照）。
- ブラウザの「決めること」に、チームが自分の判断を待っているものが並びます。1 件答えてから、「最近決めたもの」で「反映済み」になっていること、チームの側にも届いていることを確かめます。

チームランタイムだけを確かめるには、ノードと同じ形で呼び出します。

```sh
echo '{"runtime":"agency-runtime/1","org":"acme","team":"app","op":"describe"}' \
  | SCRUM_STATE_DIR=~/teams/app scrum runtime
```

JSON が 1 行出力され、その `result.runtimes` に `agency-runtime/1` が含まれていれば正常です。

## Cloudflare Access の背後に置いて外から使う

スマートフォンやほかのマシンからノードを使うとき、またはほかのノードから届くようにするときは、ノードを Cloudflare Tunnel と Cloudflare Access の背後に置きます。
ノードは引き続き `127.0.0.1` で待ち受け、同じ Mac で動くトンネルがそこへつなぎます。
クライアントは Access にサインインし、ノードはリクエストのたびに Access のトークンを検証します。
ほかのノードは Access にサインインできないため、リクエストごとに自分の鍵で署名します。
そのため、連合のルートは Access を通さずに受け付けます。

### ノードのアドレスを変える

ほかのノードは、ノードの `url` を宛先にします。
組織定義には、各チームがどのノードで動いているかが記録されています。
公開ホスト名が決まったら、`~/.agency/config.json` の `url` を `"https://agency.example.com"` に変えてノードを再起動し、チームの置き場所を移します。
この操作は組織のルート鍵を持つノードで行い、自分で作った組織ならこのノードがそれにあたります。

```sh
agency team-move --org acme --team app --node https://agency.example.com
```

移すまでのあいだも、ノードは動き続けてチームを提供します。
ほかのノードがチームを別の場所に探していることは、ログに出ます。
自分の端末からしか使わず、ほかのノードが届く必要のないノードなら、`url` はループバックのままでかまいません。

### Cloudflare Tunnel

1. `cloudflared` を入れます（`brew install cloudflared`）。
2. Cloudflare Zero Trust でトンネルを作り、公開ホスト名 `agency.example.com` を追加して、そのサービスを `http://127.0.0.1:4400` にします。
3. トンネルについての Cloudflare の手順に従い、Mac の起動時に始まるサービスとして `cloudflared` を動かします。

トンネルを通ったリクエストには Cloudflare のヘッダーが付きます。
ノードはこのヘッダーを見て外からのリクエストとして扱い、`127.0.0.1` から届いていても Access のサインインを求めます。

### Access アプリケーション

Cloudflare Zero Trust で、`agency.example.com` のセルフホスト型 Access アプリケーションを追加します。
ポリシーでは自分だけを許可します（たとえば自分のメールアドレス）。
ノードの設定に使うので、次の二つを控えておきます。

- アプリケーションの Audience（AUD）タグ。16 進数 64 文字です。
- チームドメイン。`<チーム名>.cloudflareaccess.com` の形です。

### Access を通さないパス

次の二つのパスは、Access のサインインなしでノードに届く必要があります。
それぞれのパスについて、アクションが Bypass の Access アプリケーション（またはポリシー）を追加します。

- `agency.example.com/fed/*`：連合のルートです。ほかのノードは Access にサインインできないので、ノードは代わりに署名を検証します（[protocol.md](protocol.md) の「Federation routes」を参照）。
- `agency.example.com/.well-known/apple-app-site-association`：iPhone アプリのサインインのために、Apple のサーバーがサインインなしで取得します。下の `appId` を設定するときだけ必要です。

これ以外のパスは Bypass にしないでください。
外から届いたリクエストのうち、ほかのパスへのものは、正しい Access トークンがなければノードが拒否します。

### access の設定

`~/.agency/config.json` に `access` を追加し、ノードを再起動します。

```json
"access": {
  "publicHost": "agency.example.com",
  "teamDomain": "acme.cloudflareaccess.com",
  "audience": "<16 進数 64 文字>",
  "ownerEmail": "alice@example.com"
}
```

- `ownerEmail` は、自分が Access にサインインするときのメールアドレスです。ほかのメールアドレスでサインインした人も読むことはできますが、決定や端末の追加といった書き込みができるのは `ownerEmail` だけです。
- iPhone アプリが Web ビューの外で Access にサインインできるようにするには、`"appId": "ABCDE12345.com.example.agency"`（`<チーム ID>.<バンドル ID>`）を追加し、上の二つ目の Bypass も設定します。詳しくは [apple.md](apple.md) を参照してください。
- 新しい依頼を iPhone に通知するには、アプリの APNs キーを指定した `push` の設定を追加します。書き方は README にあります。

設定の形式が正しくないと、起動時にログに出ます（[トラブルシューティング](#トラブルシューティング)を参照）。
その場合、ノードは Cloudflare を通ったリクエストをすべて拒否します。
ノードが何を検証するかは、[protocol.md](protocol.md) の「Who is let in」に詳しく書かれています。

## スマートフォンやブラウザを端末として追加する

自分の代わりに決定するブラウザやスマートフォンは、それぞれ自分の鍵を作って持ちます。
鍵が端末の外に出ることはありません。

1. 新しい端末で Web クライアントを開きます。ノードの Mac なら `http://127.0.0.1:4400/`、それ以外なら Access にサインインしてから `https://agency.example.com/` です。iPhone アプリも同じページを開きます（[apple.md](apple.md)）。
2. 「端末」を開き、「この端末を追加」を選びます。照合コードが表示されます。
3. 次のどちらかでコードを確かめて承認します。
   - 追加済みの端末で「端末」を開き、依頼を見つけ、コードが一致すれば承認します。
   - ノードのターミナルで `agency device-add` を実行します。端末がまだ一つもなければノードのアカウント鍵で、追加済みの CLI の端末があればそれで署名します。

依頼は 1 時間有効で、ノードが保持するのは 10 件までです。
ブラウザが鍵を作れるのは、`https` のページか、ノードの Mac 上の `127.0.0.1` のページだけです。
それ以外では、鍵を作れないと画面に表示されます。

## ノードの更新

```sh
cd ~/src/agency
git pull
(cd web && npm ci && npm run build)
cargo build --release -p agency-node
cp target/release/agency ~/.local/bin/agency.new && mv ~/.local/bin/agency.new ~/.local/bin/agency
launchctl kickstart -k gui/$(id -u)/local.agency.node
```

データディレクトリには手を加えません。
ノードは起動のたびに書き込み用のトークンを作り直すため、開いていたページは次の書き込みの前に一度読み込み直します。

## トラブルシューティング

### ログの行

ノードは `~/.agency/logs/serve.log` に次の行を書きます。

| 行 | 意味 |
|---|---|
| `Node running: <url> (listening on <address>)` と `Web client: …` | ノードが起動しました。 |
| `agency: cannot listen on 127.0.0.1:4400: Address already in use` | ほかのプロセスがポートを使っています。launchd は 10 秒ごとに起動し直します。`lsof -nP -iTCP:4400 -sTCP:LISTEN` でどのプロセスかわかります。 |
| `agency: team acme/app runs on this node (…), but the org places it at …` | ノードの `url` が変わっています。行に書かれた `agency team-move` を実行します。 |
| `agency: the access setting is malformed (…)` | `config.json` の `access` を直します。直すまで、Cloudflare を通ったリクエストはすべて拒否されます。 |
| `agency: the access appId is malformed (…)` | `appId` を直します。`access` のほかの部分は有効ですが、アプリのサインインは使えません。 |
| `agency: refused an Access request (no Access token)` | Access のサインインがないリクエストが Cloudflare から届きました。Bypass が二つのパス以外にも及んでいるか、ホスト名の前に Access がありません。 |
| `agency: refused an Access request (wrong audience)` | `audience` がアプリケーションの AUD タグと違います。 |
| `agency: refused an Access request (wrong issuer)` | `teamDomain` が自分のチームドメインと違います。 |
| `agency: refused an Access request (expired)` または `(not valid yet)` | サインインの期限が切れたか、この Mac の時計がずれています。 |
| `agency: refused an Access request (cannot fetch the Access certs: …)`、`(unknown signing key)`、`(the signature does not match)` | ノードが `teamDomain` から Access の鍵を取得できないか、トークンがこのチームドメインのものではありません。 |
| `agency: push notifications are off: …` | `push` のキーを読めません。通知は送られません。 |
| `agency: a push notification was not sent: …` | APNs が通知を受け付けませんでした。30 秒後にもう一度送ります。 |
| `agency: re-reading the replicas failed: …`、`agency: checking for new requests failed: …`、`agency: retrying the outbox failed: …` | ノードがデータディレクトリに書き込めませんでした（ディスクがいっぱいなど）。ほかのノードに届かないことはログに出ません。送信待ちのものは `agency status` に、ほかのノードにあるチームの写しをいつ読んだかは Web クライアントに表示されます。 |
| `agency: <メッセージ>` のあとノードが止まる | `config.json` を読めないなどの理由で起動できませんでした。launchd は 10 秒ごとに起動し直します。 |

### ノードの応答

ログには出ず、ブラウザや `curl` で見えるノードの応答にだけ現れる問題もあります。

| 応答 | 意味 |
|---|---|
| `503` `cannot start the team runtime: No such file or directory` | `config.json` に書かれたパス（`teams[].runtime.command`）にチームランタイムのコマンドがありません。 |
| `503` `the team runtime did not answer within 20 seconds` | チームランタイムが止まっているか、応答が遅すぎます。 |
| `503` `the team runtime did not write an agency-runtime/1 response (…): …` | チームランタイムが失敗しました。応答の末尾は、チームランタイムが標準エラーに書いた内容です。launchd の `PATH` にツールがないことがよくある原因です。 |
| `421` `wrong host` | `127.0.0.1:<ポート>` か `localhost:<ポート>` 以外のアドレスでページを開いています。 |
| `403` `the Client API is local only` | ほかのマシンからのリクエストが、Cloudflare を通らずにノードへ届きました。 |
| `403` `missing or wrong write token` | ページがノードの最後の起動より前に開かれたものです。読み込み直してください。 |
| `403` `only the node's owner may decide` | `ownerEmail` 以外のメールアドレスで Access にサインインしています。 |

## チームランタイム

ノードはハーネスのファイルを読みません。
チームごとにチームランタイムのコマンド（`SCRUM_STATE_DIR` にチームの状態ディレクトリを設定した `<scrum> runtime`）を実行し、agency-runtime/1 でやり取りします。
詳細は [protocol.md](protocol.md#agency-runtime1) にあります。
ハーネス自体の準備は、ハーネスの手順に従ってください。
ノードがハーネスに求めるのは次のことです。

- agency-runtime/1 の `describe`、`view`、`decide` に答えるコマンド。費用の画面を使うなら `tokenUsage` にも答えます。どの呼び出しにも 20 秒以内に答える必要があります。
- `team-add --state-dir` に渡したものと同じ、チームの状態ディレクトリ。
- チームランタイムが呼ぶすべてのツールが、launchd のエージェントに書かれた `PATH` から見つかること。
- 自分あての依頼は、`to` を省くか `owner` にすること。自分のほかにメンバーがいない組織では、すべての依頼が自分あてになります。自分を待っている参加者は、`waitingOn` が `{ "kind": "person", "person": "owner" }` になります。

ノードは動いているあいだ、30 秒ごとと、ページがチームを読むたびに、チームランタイムにビューを問い合わせます。
そのため、`view` は軽い処理にしておく必要があります。
