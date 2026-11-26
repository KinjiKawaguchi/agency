# ノードのインストール

この文書では、Agency のノードを Mac か Linux のマシンにソースから、またはリリースビルドからインストールし、launchd のエージェントか systemd のユニットとして動かし、最初の端末を追加するまでを説明します。
続いて、そのノードを Cloudflare Access の背後に置き、ほかのマシンから使えるようにする方法を説明します。
対象は macOS（launchd）と Linux（systemd）で、手順が違う箇所にはどちらの手順かを書いています。

英語版は [install.md](install.md) で、節の構成は同じです。
内容が食い違うときは英語版が正です。

例ではアカウント `alice`、組織 `acme`、チーム `app`、ホスト名 `agency.example.com` を使います。
実際には自分の名前に置き換えてください。
ID に使えるのは英小文字、数字、ハイフンです（`[a-z0-9][a-z0-9-]{0,39}`）。

## 必要なもの

- macOS（Apple シリコンでも Intel でもかまいません）、または glibc 2.35 以降の Linux（x86_64 か arm64）。ノードをサービスとして動かすには systemd も要ります。
- macOS では Xcode Command Line Tools。`xcode-select --install` で入ります。Linux では C コンパイラと `curl`。たとえば `sudo apt install build-essential curl` で入ります。
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

できたバイナリは、再ビルドや `cargo clean` の影響を受けない場所に置きます。Cargo はバイナリを、リポジトリの `target/release/` に、`CARGO_TARGET_DIR` を設定しているときは `$CARGO_TARGET_DIR/release/` に作ります。
いったん別名でコピーしてから名前を変えるのは、動いているノードのバイナリをその場で書き換えないためです。

```sh
mkdir -p ~/.local/bin
cp target/release/agency ~/.local/bin/agency.new && mv ~/.local/bin/agency.new ~/.local/bin/agency
~/.local/bin/agency --help
```

以下ではこれを `agency` と書きます。
`~/.local/bin` を `PATH` に入れるか、フルパスで実行してください。
`agency` は `--dir` を指定しないと `$AGENCY_DIR` に、それも未設定なら `~/.agency` にデータを置きます。

### リリースビルド

ソースからビルドする代わりに、リリースビルドも使えます。リポジトリの Releases ページには、リリースごとに、macOS 用（Apple シリコンの `arm64` と Intel の `x86_64`）と Linux 用（`x86_64` と `arm64`）の tarball があります（`uname -m` は、`x86_64` か、Linux の arm64 では `aarch64` を表示します）。Linux のビルドは、glibc 2.35 以降のシステムで動きます。リリースの作り方は [releasing.md](releasing.md) にあります。

1. お使いのマシンに合った tarball とその `.sha256` を、リリースからダウンロードします。GitHub CLI なら `gh release download <タグ> --repo <このリポジトリ> --pattern 'agency-*-<os>-<arch>.tar.gz*'` です。`<os>` は `macos` か `linux` です。リリースのものではないビルドは、**Actions** タブの **release** ワークフローのアーティファクトです（名前は `agency-<バージョン>-<コミット>-linux-<arch>`）。
2. 確かめて展開し、前の節と同じ方法でバイナリを置きます。

```sh
shasum -a 256 -c agency-*.tar.gz.sha256   # Linux では sha256sum -c
tar xzf agency-*.tar.gz
mkdir -p ~/.local/bin
cp agency-*/agency ~/.local/bin/agency.new && mv ~/.local/bin/agency.new ~/.local/bin/agency
```

tarball には、ノードをサービスとして動かすスクリプト（macOS では `launchd-plist.sh` と `deploy-macos.sh`、Linux では `systemd-unit.sh`。[launchd で動かす](#launchd-で動かす)と[systemd で動かす](#systemd-で動かすlinux)を参照）、この文書、ライセンスも入っています。バイナリには Web クライアントがすでに入っています。

macOS のビルドは、署名も公証もしていません。ブラウザでダウンロードしたファイルは macOS が隔離し、そのままでは開けません。`xattr -d com.apple.quarantine ~/.local/bin/agency` で印を外してください（`gh release download` と `curl` では印は付きません）。

## ローカル専用のノード

ローカル専用のノードは、1 台のマシンの中だけで動きます。
待ち受けるのは `127.0.0.1` だけなので、Web クライアントはそのマシンでしか開けず、ノードもそのマシンからのリクエストしか受け付けません。
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
- `org-create` は、新しい組織のためのルート鍵を作り、このノードをそのルートのノードにします。`--node` を付けない `team-add` は、このノードのアカウントを持ち主として、チームをこのノードで動かします。

`agency status` に、アカウント、`Org acme (Acme), version 2, root key`、`Team on this node: acme/app (owner alice) runs <scrum> runtime` が表示されれば成功です。

データディレクトリには、ノードのアカウント鍵である `account.key` ができています。
この鍵はノードの読み取り、参加の依頼、組織での最初の端末の追加に署名するので、このファイルを読める人は、あなたが読めるものを読めます。
組織のすべての版（メンバー、端末、チーム）に署名する組織のルート鍵は、これとは別に `~/.config/agency/root-keys/` に置かれます（組織と鍵ごとに 1 ファイルで、ファイルの持ち主だけが読めます）。読み出されるのは版に署名するときだけです。
どちらもこのマシンの外に持ち出さず、同期や共有をするフォルダにも置かないでください。
データディレクトリのバックアップには、ルート鍵は入っていません。ルート鍵がなくなると誰も組織を変更できなくなるので、別にバックアップしてください。`~/.config/agency/root-keys/` を、オフラインで暗号化された保管先（たとえば、この Mac の外に置く暗号化ディスクイメージ）にコピーします。`agency root-rotate` は新しい鍵を作って古い鍵を消すので、そのあとにもう一度コピーしてください。
ルート鍵を分ける前に作った組織は、`agency root-rotate --org acme` で専用の鍵に移すまで、`account.key` をルート鍵として使います。

### launchd で動かす

エージェントの定義ファイルは `scripts/launchd-plist.sh` で作ります。
エージェントにはスクリプトを実行したシェルの `PATH` が渡るので、チームランタイムはターミナルと同じツールを見つけられます。
`scrum` とハーネスのツールが動くターミナルで実行してください。

この `PATH` は、スクリプトを実行した時点のシェルの値の写しです。チームランタイムが実行するコマンドは、すべてその `PATH` のどれかのディレクトリになければなりません。scrum のハーネスなら、`scrum` のほかに `node` と `gh` も必要です（そのターミナルで `command -v scrum node gh` を実行して確かめます）。あとから入れたり動かしたりしたツールや、スクリプトを実行したシェルが読み込まなかったファイルでターミナルの `PATH` を設定している場合、そのコマンドはエージェントにありません。するとランタイムは `spawnSync gh ENOENT`（`ENOENT` は「見つからない」の意味）で失敗し、ノードは `a command the team runtime runs was not found` とログに書きます。そのときは、すべてが動くターミナルから、スクリプトを実行し直してください。

```sh
scripts/launchd-plist.sh --agency ~/.local/bin/agency --dir ~/.agency \
  --out ~/Library/LaunchAgents/local.agency.node.plist
plutil -lint ~/Library/LaunchAgents/local.agency.node.plist
launchctl bootstrap gui/$(id -u) ~/Library/LaunchAgents/local.agency.node.plist
```

エージェントはログイン時に `agency serve` を起動し、止まったら起動し直します。
ノードのログは `~/.agency/logs/node.jsonl` です（[ログ](#ログ)を参照）。`~/.agency/logs/serve.log` には、エージェントが受け取るもの（起動時の行とクラッシュ）が残ります。

| したいこと | コマンド |
|---|---|
| 動いているか確かめる | `launchctl print gui/$(id -u)/local.agency.node \| grep state` |
| 再起動する | `launchctl kickstart -k gui/$(id -u)/local.agency.node` |
| 次に読み込むまで止める | `launchctl bootout gui/$(id -u)/local.agency.node` |

ハーネスの置き場所を変えたときなど、`PATH` が変わったら、スクリプトを実行し直してから `bootout` と `bootstrap` をやり直します。

### systemd で動かす（Linux）

ユニットの定義ファイルは `scripts/systemd-unit.sh` で作ります（リリースの tarball では `systemd-unit.sh` という名前で入っています）。
ユニットにはスクリプトを実行したシェルの `PATH` が渡るので、チームランタイムはターミナルと同じツールを見つけられます。systemd の `--user` のサービスには、そのままでは短い既定の `PATH` しか渡らないためです。
`scrum` とハーネスのツールが動くターミナルで実行してください。

`node` や `gh` のように、チームランタイムが実行するコマンドがすべてこの `PATH` にあることも、上の launchd の節と同じく必要です。

```sh
scripts/systemd-unit.sh --agency ~/.local/bin/agency --dir ~/.agency \
  --out ~/.config/systemd/user/agency-node.service
systemctl --user daemon-reload
systemctl --user enable --now agency-node
```

ユニットはログイン時に（次のとおり linger を有効にすれば、マシンの起動時に）`agency serve` を起動し、止まったら 10 秒後に起動し直します。
ノードのログは `~/.agency/logs/node.jsonl` です（[ログ](#ログ)を参照）。`~/.agency/logs/serve.log` には、ユニットが受け取るもの（起動時の行とクラッシュ）が残ります。systemd が起動と停止をいつ行ったかは、`journalctl --user -u agency-node` でわかります。

| したいこと | コマンド |
|---|---|
| 動いているか確かめる | `systemctl --user status agency-node` |
| 再起動する | `systemctl --user restart agency-node` |
| 次に起動するまで止める | `systemctl --user disable --now agency-node` |

ハーネスの置き場所を変えたときなど、`PATH` が変わったら、スクリプトを実行し直してから `systemctl --user daemon-reload` と `systemctl --user restart agency-node` を実行します。

systemd がサービスを動かすのは、ログインセッションがあるあいだだけです。マシンの起動時にノードを始め、ログアウトしても動かし続けるには、一度だけ linger を有効にします。

```sh
loginctl enable-linger "$USER"
loginctl show-user "$USER" --property=Linger
```

2 つ目のコマンドは `Linger=yes` を出力するはずです。有効でないと、最後のセッションからログアウトした時点でノードが止まります。
`systemctl --user` はログインセッション（ターミナルや SSH のログイン）で実行してください。`su` や `sudo -u` からでは自分のサービスに届かず、`Failed to connect to bus` と表示されます。

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
agency logs --since 10m
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
ノードは引き続き `127.0.0.1` で待ち受け、同じマシンで動くトンネルがそこへつなぎます。
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

1. `cloudflared` を入れます（macOS では `brew install cloudflared`。Linux では、お使いのディストリビューション向けの Cloudflare のパッケージです）。
2. Cloudflare Zero Trust でトンネルを作り、公開ホスト名 `agency.example.com` を追加して、そのサービスを `http://127.0.0.1:4400` にします。
3. トンネルについての Cloudflare の手順に従い、マシンの起動時に始まるサービスとして `cloudflared` を動かします。

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
- `access` を設定すると、ノードの Mac 上の `http://127.0.0.1:4400/` は読むためだけのものになります。決定や端末の追加は、その Mac の上でも Access にサインインして `https://agency.example.com/` から行います。同じ Mac のほかのプログラムが、あなたとして書き込めないようにするためです。
- iPhone アプリが Web ビューの外で Access にサインインできるようにするには、`"appId": "ABCDE12345.com.example.agency"`（`<チーム ID>.<バンドル ID>`）を追加し、上の二つ目の Bypass も設定します。詳しくは [apple.md](apple.md) を参照してください。
- 新しい依頼、追加を待つ端末、拒否された決定を iPhone に通知するには、アプリの APNs キーを指定した `push` の設定を追加します。書き方は README にあります。タイトルやラベルを Apple に送りたくないときは、`push` に `"hideTitles": true` を加えます。
- ブラウザにも通知するには（Android、Windows、Linux、macOS の Chrome、Edge、Firefox、Mac の Safari、インストールしたアプリ。iPhone のブラウザは対象外で、iPhone には専用のアプリがあります）、`config.json` に `"webPush": {}` を加えて再起動します。ノードがデータディレクトリに自分の鍵（`web-push-vapid.key`。アカウント鍵と同じように人に見せないでください）を作り、**端末**の画面にブラウザごとに通知を入れるボタンが出ます。通知は、そのブラウザのプッシュサービス（Google、Mozilla、Apple、Microsoft）を通り、ブラウザ宛てに暗号化されます。`"subject"`（プッシュサービスが連絡できる `mailto:` か `https://` のアドレス。省略するとノードのホスト名になります。`access` がないときはノード自身のアドレスになり、Apple のプッシュサービスはそれを受け付けないので、その場合は `mailto:` のアドレスを指定してください）と、`"extraHosts"`（許すプッシュサービスを増やす）は任意です。タイトルやラベルをどのサービスにも送りたくないときは（Apple にも）、`config.json` の一番上に `"hideTitles": true` を加えます。iPhone への通知でも隠れます。

設定の形式が正しくないと、起動時にログに出ます（[トラブルシューティング](#トラブルシューティング)を参照）。
その場合、ノードは Cloudflare を通ったリクエストをすべて拒否します。
ノードが何を検証するかは、[protocol.md](protocol.md) の「Who is let in」に詳しく書かれています。

## スマートフォンやブラウザを端末として追加する

自分の代わりに決定するブラウザやスマートフォンは、それぞれ自分の鍵を作って持ちます。
鍵が端末の外に出ることはありません。

1. 新しい端末で Web クライアントを開きます。ノードのマシンなら `http://127.0.0.1:4400/`、それ以外なら Access にサインインしてから `https://agency.example.com/` です。iPhone アプリも同じページを開きます（[apple.md](apple.md)）。
2. 「端末」を開き、「この端末を追加」を選びます。照合コードが表示されます。
3. 次のどちらかでコードを確かめて承認します。
   - 追加済みの端末で「端末」を開き、依頼を見つけ、コードが一致すれば承認します。
   - ノードのターミナルで `agency device-add` を実行します。端末がまだ一つもなければノードのアカウント鍵で、追加済みの CLI の端末があればそれで署名します。

依頼は 1 時間有効で、ノードが保持するのは 10 件までです。
ブラウザが鍵を作れるのは、`https` のページか、ノードのマシン上の `127.0.0.1` のページだけです。
それ以外では、鍵を作れないと画面に表示されます。

## Web クライアントをアプリとして入れる

Web クライアントはアプリとして入れられます。
ネイティブのビルドがなくても、Android、Windows、Linux で使えます。
専用の窓で開き、専用のアイコンが付き、入れたブラウザのサインインをそのまま使います。
**端末**の画面に、ブラウザが対応していれば「Agency を入れる」が出ます（Android、Windows、Linux、Mac の Chrome と Edge）。
Mac の Safari では、ファイルメニューから「Dock に追加」を選びます。
iPhone には専用のアプリがあります（[apple.md](apple.md)）。

**端末**の画面の「このブラウザの通知」には、スイッチが 2 つあります。1 つ目は、ページが隠れているあいだ、新しいことをオペレーティングシステムの通知で知らせます。2 つ目の「アプリを閉じているとき」は、ノードに `webPush`（下を見てください）があり、ブラウザが Web Push に対応しているときだけ出ます（iPhone のブラウザは対象外で、iPhone には専用のアプリがあります）。ページを開いていなくても、ノードがブラウザのプッシュサービスを通して知らせます。許可はスイッチを押したときにブラウザが尋ね、1 つの許可が両方に使われます。ブラウザごとに別々に登録され、ホームノードごとに登録は別です。ノードの鍵が作り直されたときは、画面が登録し直すよう伝えます。

入れたアプリが覚えるのは、Web クライアントのビルド済みファイル（`/assets/` と `/icons/`）だけです。
開くのを速くするためです。
ページは覚えません。ページにはノードの書き込みトークンが入っているからです。
API の答えも覚えません。組織のデータが入っているからです。
ノードに届かないときは、届かないと伝えるページを出します。
サインインが切れたら、覚えたものを消します。

Cloudflare Access の背後では、ノードは、マニフェストやアイコンを含むすべての要求にサインインを求めます。
そのため、ページはブラウザの資格情報つきでマニフェストを取りに行きます。
このために Access を通さないパスを増やす必要はありません。
ノードをデプロイしたあと、次の手順で確かめます。

1. Chrome（Android とデスクトップ）で `https://agency.example.com/` を開き、サインインして「端末」を開きます。「Agency を入れる」が出ます。
2. 入れます。アプリが専用の窓で決めること（デスク）を開きます。印鑑のアイコンが見えて（空白でなく）、サインイン済みです。
3. アプリを閉じ、Access からサインアウトします（またはセッションが切れるのを待ちます）。もう一度アプリを開くと、古いページではなく Access のサインインが出ます。
4. ネットワークを切ってアプリを開くと、ノードに届かないと表示されます。

手順 2 でアイコンが空白なら、ブラウザがアイコンの要求にサインインを付けませんでした。
チームに知らせてください。アイコンをマニフェストに埋め込みます。

## AI サブスクリプションと使用量

概要画面には、ノードの資金源と、それぞれのサブスクリプションの上限をどれだけ使ったかが並びます。
資金源は一人の人が自分で契約しているサブスクリプションです。ノードはその人自身のログインで読み、別のアカウントと共有することはありません。
ノードは、Claude のサブスクリプションの使用量を自分で読めます。マシンにログインがあるだけ、いくつでも読めます。

### 1 台のマシンで複数の Claude にログインする

Claude Code は、設定ディレクトリごとに 1 つのログインを持ちます。設定ディレクトリは `CLAUDE_CONFIG_DIR` として渡したものです。ログインを増やすには、専用のディレクトリを用意し、そこで一度ログインします。

```sh
CLAUDE_CONFIG_DIR=$HOME/.claude-accounts/personal-2 claude
```

開いたら `/login` を実行します。そのログインを使うときは、いつも同じ `CLAUDE_CONFIG_DIR` を付けて Claude Code を起動します。

Claude Code はログインを次の場所に保存します。

| マシン | 保存先 |
|---|---|
| macOS | キーチェーンの項目 `Claude Code-credentials-<16 進 8 文字>`。`CLAUDE_CONFIG_DIR` に設定した文字列の SHA-256 の先頭 8 桁です。`CLAUDE_CONFIG_DIR` を設定しない場合（`~/.claude` のログイン）は `Claude Code-credentials` です。キーチェーンが書き込みを断ったときは、次のファイルを使います。 |
| Linux | `<ディレクトリ>/.credentials.json`（モード `0600`）。`CLAUDE_CONFIG_DIR` がなければ `~/.claude/.credentials.json` です。 |

数えられるのはディレクトリの文字列そのものです。末尾にスラッシュを付けた `/home/alice/.claude-work/` は、`/home/alice/.claude-work` とは別の項目になります。
ノードの設定には、`CLAUDE_CONFIG_DIR` に設定したとおりの文字列を、絶対パスで書いてください。

### 使用量を読む

`~/.agency/config.json` に資金源を並べ、ノードを再起動します。設定には秘密を書きません。

```json
"funding": [
  { "id": "personal-1", "label": "Claude (personal 1)", "kind": "subscription",
    "collector": { "type": "claude" } },
  { "id": "personal-2", "label": "Claude (personal 2)", "kind": "subscription",
    "collector": { "type": "claude", "configDir": "/Users/alice/.claude-accounts/personal-2" } }
]
```

- `collector` を書くと、ノード自身がその資金源を読みます。`configDir` を省くと、`~/.claude` のログインを読みます。
- 使用量はデータディレクトリの `usage/<id>.json` に書かれます。資金源に `usageFile` があるときは、そこに書かれます。`usageFile` だけの資金源は、これまでどおり別のツールが保ちます。
- ノードは資金源ごとに、必要なときにそのログインのアクセストークンを読み、`https://api.anthropic.com/api/oauth/usage` に 5 時間窓と 7 日窓を問い合わせ、その 2 つの窓だけを書きます。トークンは保持せず、ファイルにもログにも書きません。
- 同じ資金源への問い合わせは、5 分に 1 回までです。失敗すると、そのたびに待ち時間を倍にし、最長 1 時間まで延ばします。サービスがもっと長く待つよう求めたときは、それに従います。読み取りに失敗しても最後のファイルは残り、概要画面にはファイルを最後に書いた時刻が出ます。
- ノードは、トークンを自分では更新しません。更新トークンは 1 回使うと新しいものに差し替わるので、同じログインを 2 つのプログラムが同時に更新すると、片方の持つトークンが使えなくなり、Claude Code はそのログインを忘れます。アクセストークンは数時間もち、そのログインで Claude Code を動かしたときに Claude Code が更新します。誰も使わないログインは期限切れのままで、使用量は `unknown` になり、そこへ移されることもなくなるので、ノードが `claude` に更新させます。
  - ログインのトークンが期限切れから 10 分たつと（そのログインを使うセッションがあれば、それまでに更新されています）、ノードは `claude -p "Reply with the single word: ok" --model haiku --no-session-persistence` を 1 回、そのログインの `CLAUDE_CONFIG_DIR` で、空のディレクトリ（データディレクトリの `renewals/work`）から実行します。`claude` がログインではなく別の認証を使わないよう、`ANTHROPIC_API_KEY`、`ANTHROPIC_AUTH_TOKEN`、`CLAUDE_CODE_OAUTH_TOKEN` は外します。60 秒で止め、出力は捨て、ログインを読み直します。認証情報にはノード自身は触れません。
  - 1 つのログインに対する上限は、3 時間に 1 回、24 時間に 4 回までで、失敗した実行のあとは 6 時間実行しません。実行の記録はデータディレクトリの `renewals/` にあるので、ノードを再起動しても数え直しにはなりません。1 回の実行で、サブスクリプションのトークンを数個使います。実行するとログに出ます（event は `usage` で、`running claude once to renew an expired login`）。失敗した実行は、読み取りの失敗の行に理由とともに出ます。`claude` が見つからない（launchd のエージェントや systemd のユニットの `PATH` にそのディレクトリを入れるか、`command` を設定します）、終わらない、エラーで終了した（多くは、ログインが拒否された場合で、その `CLAUDE_CONFIG_DIR` で `claude` を起動して `/login` をやり直します）のどれかです。
  - Claude Code とぶつかりうるのは、同じログインで開いているセッションが、実行と同じ秒に更新する場合だけです。これは、2 つの Claude Code のセッションがお互いに起こす衝突と同じです。10 分待つので、起こりにくくなっています。資金源の `collector` に `"renew": false` を書くと、実行を止められます。`"command": "/path/to/claude"` でプログラムを指定します（既定は、ノードの `PATH` にある `claude`）。
- macOS では、キーチェーンの項目を最初に読むとき、`security` に使用を許可するか尋ねられます。**常に許可** を選んでください。ノードは答えを 20 秒待ち、あきらめて、あとでもう一度試します。

ChatGPT のプランを Codex 経由で使っている場合は、ChatGPT でログインし（`codex login`）、資金源として並べます。

```json
{ "id": "codex", "label": "ChatGPT (Codex)", "kind": "subscription",
  "collector": { "type": "codex" } }
```

- `codexHome` には、別のログインの `CODEX_HOME` を書きます（既定は `~/.codex`）。`command` には、ノードの動く `PATH` に `codex` がないときの、`codex` のパスを書きます。
- ノードは `codex app-server` を起動し、プランのレート制限（`account/rateLimits/read`）を尋ねます。5 時間窓と週の窓が、Claude と同じ間隔で使用量ファイルに書かれます。
- Codex は自分でログインを持つので、ノードは Codex のトークンを読みません。ログインの更新は、Codex が自分の実行のときと同じように行います。
- API キーのログインにはプランの窓がなく、ログがそう伝えます。Codex は `app-server` を実験的な機能としています。確かめたのは Codex CLI 0.151 です。

### 新しいセッションで使うサブスクリプションを選ぶ

エージェントのセッションを起動するプログラムは、新しいセッションをどのサブスクリプションで動かすかを、ノードに尋ねられます。`funding` の並び順がそのまま優先順で、`collector` を持つ資金源ごとに、7 日窓のしきい値を決められます。

```json
{ "id": "personal-1", "label": "Claude (personal 1)", "kind": "subscription",
  "sevenDayThreshold": 90, "collector": { "type": "claude" } }
```

`sevenDayThreshold` は 1 から 100 までの百分率です（既定は 80）。

```sh
agency funding pick --tool claude
```

JSON を 1 つ出力します。`source` が使う資金源、`env` がセッションに設定する環境変数（`CLAUDE_CONFIG_DIR`。`--tool codex` では `CODEX_HOME`。既定のログインでは空）、`reason` がその理由です。`state` は次のとおりです。

| `state` | 意味 |
|---|---|
| `ok` | 7 日窓がしきい値を下回っている、先頭の資金源です。 |
| `unknown` | しきい値を下回っていると分かる資金源がなく、この資金源の使用量が読めません（ファイルが 30 分より古い、または窓がすでに更新されています）。読めないことは、超えていることとは違います。 |
| `overThreshold` | 読める資金源がすべてしきい値を超えています。これは 100% までの余裕が最も大きい資金源です。セッションは続け、概要画面でそのことを伝えます。 |
| `none` | すべての資金源が上限に達しています（どれかの窓が 100）。`source` はなく、コマンドは終了コード 3 で終わります。 |

`agency funding list` は、同じ資金源を優先順に、`env` としきい値、いまの状態とともに出力します。すべての設定ディレクトリと、どのログインがしきい値を超えているかを知る必要があるプログラム向けです。資金源ごとに 3 つの旗があり、いつも出力されます。

| 旗 | 真になるとき |
|---|---|
| `overThreshold` | 7 日窓がしきい値に達しているか超えている（100 のときも） |
| `blocked` | プラン自身の上限に達している。7 日窓が 100、または 5 時間窓が 100 でまだ更新されていない |
| `unknown` | いま使用量を読めない（ファイルがない、30 分より古い、窓がすでに更新されている）。このとき、ほかの 2 つは偽です。読めないことは、超えていることとは違うからです |

動いているセッションをログインから移すプログラムは、そのログインが `overThreshold` のときだけ移し、`blocked` や `overThreshold` のログインへは移さないでください。`pick` も、選んだ資金源について同じ旗を出力します。どちらのコマンドも、動いているノードもアカウントの鍵も要らず、秘密は出力しません。`env` にあるのはパスです。

動いているセッションは、始めた資金源のままです。移るのは新しいセッションだけです。使用量は最長 5 分遅れるので、資金源はしきい値を、5 分間の作業で使う分だけ超えることがあります。しきい値は 100 より低めにしてください。

資金源はそれぞれ一人の人が自分で契約しているサブスクリプションで、ノードが受け持つアカウントは 1 つです。したがって、切り替えは一人の人のサブスクリプションの中で行われます。ログインを別の人と共有したり、複数の人のログインをまとめて使ったりしてはいけません。Claude の個人向け規約は、アカウントの共有を禁じています。また、切り替えは上限を回避する手段ではありません。上限に達した資金源は、窓が更新されるまで選ばれません。

## 配送メトリクス

評価の画面には、各チームがどれだけ速く出荷しているかが出ます。種類別にマージした PR の数、開いてからマージするまでの時間、デフォルトブランチの CI が赤だった時間、リバート、マージした PR 1 件あたりのトークンの費用です（すべての数の定義は [protocol.md](protocol.md#evaluation) にあります）。チームが動いているノードが、そのチームの GitHub のリポジトリから数を計算し、ほかのノードはそこから読みます。

チームで有効にするには、`~/.agency/config.json` の `teams[].delivery` にリポジトリ（`owner/name`）を書き、ノードを再起動します。

```json
"teams": [
  { "org": "acme", "team": "app", "owner": "owner",
    "runtime": { "command": ["scrum", "runtime"] },
    "delivery": { "repos": ["acme/app", "acme/web"] } }
]
```

- ノードは、そのマシンの `gh` のログインで、読み取りだけを行って GitHub を読みます。使うのは `gh api` の読み取りだけです（マージされた PR、その PR が閉じる Issue と、その Issue がボードで In Progress に動いた時刻の GraphQL クエリ、デフォルトブランチのワークフロー実行の読み取り）。トークンは保持せず、設定にも書きません。`gh` が入っていて、launchd のエージェントか systemd のユニットの `PATH`（「launchd で動かす」を参照）にあり、ノードを動かすアカウントでログインしている（`gh auth login`）必要があります。`GH_HOST` など `gh` の環境変数は、`gh` のとおりに働きます。
- 読み取りしかできないログインを使うほうが安全です。`gh auth login --with-token` に、リポジトリの Pull request、Issue、Actions、メタデータへの読み取り権限と、プロジェクト（Issue が In Progress に動いた時刻のため）への読み取り権限をもつ、きめ細かいトークンを渡します。
- チームのランタイムが参加者を報告する（`participantMetrics`）と、チームの数には、参加者ごとに、セッションが働いた時間、人の判断を待った時間、空いていた時間、トークンの費用、マージされた PR のうち参加者のものの数（`merged`。ランタイムが作者を知らないものは `unattributedMerged`）も入ります。ノードは、同じ日数を自分の時間帯で尋ねます。この op がないとき、またはランタイムの答えが尋ねたものと違うときは、参加者は出さず、ほかは同じです。人の判断を待った時間は、ランタイムが一部の待ちしか見えない場合は下限です。
- GitHub の答えは、チームと期間ごとに 10 分間保持するので、画面を読んでもログインのレート制限を使いません。ディスクには何も書きません。
- `delivery` のないチームや、数を読めないチームはエラーではありません。画面は、そのチームを理由とともに並べます。

| 画面に出る理由 | すること |
|---|---|
| no repositories are configured for this team | チームが動いているノードで、チームに `delivery` を足します。 |
| the GitHub CLI (gh) is not installed on the team's node | 入れて、エージェントやユニットの `PATH` にそのディレクトリを入れます。 |
| the GitHub CLI (gh) on the team's node is not logged in | そのノードを動かすアカウントで `gh auth login` を実行します。 |
| GitHub's rate limit is used up for now | 待ちます。ノードは 10 分後にもう一度尋ねます。 |
| GitHub did not answer in time, or did not answer the request | そのマシンのネットワークと、ログインがリポジトリを読めることを確かめます。 |
| the team's node does not give delivery numbers | そのノードがこの機能より古いので、更新します。 |
| the team's node does not give delivery numbers to this account | このアカウントは組織の answerer で、読めません。 |
| a repository of the team is not written owner/name | チームが動いているノードの設定の `delivery.repos` を直します。 |
| the team's node did not answer | そのノードが止まっているか、届きません。 |

## 事業の数字

何を測るか（KPI）は組織定義に書かれますが、数字は含まれません。数字そのもの（売上、継続率、リードタイムなど）は事業のデータで、ノードのデータディレクトリの `outcomes/<組織>.jsonl` に、数字 1 件につき 1 行で保存されます。ディレクトリは所有者だけが読める権限（`0700`、ファイルは `0600`）です。ノードが返す他のどの答えにも数字は含まれず、読むのも入力するのもノードの所有者だけです。`agency outcome-add --org acme --kpi revenue --date 2026-11-24 --value 1350000` で 1 件入力し、`agency outcome-list --org acme --kpi revenue` でその KPI の数字を一覧します。

- **このディレクトリをバックアップし、共有するものには入れないでください。** データディレクトリのバックアップには数字が含まれるので、アカウント鍵と同じように扱います。
- 数字は指標の動きを読むためのもので、会計用ではありません。
- **読み取り。** `GET /api/orgs/<組織>/outcomes/readings`（数字と同じく所有者だけ）は、チームの計画が epic を狙わせた KPI が、その epic の出荷日の前後でどうだったかを返します。チームの view は完了した epic を限られた間（scrum harness では 30 日）しか載せないため、ノードは、狙われた epic が完了したと最初に見た日を `outcomes/<組織>.ships.jsonl`（同じディレクトリ、ファイルは `0600`。チーム、epic、時刻を持ち、数字は含みません）に書き、変更しません。数字と一緒に保管し、同じようにバックアップしてください。これがないと、チームの view から消えた epic の読み取りはできません。

## ノードの更新

```sh
cd ~/src/agency
git pull
(cd web && npm ci && npm run build)
cargo build --release -p agency-node
cp target/release/agency ~/.local/bin/agency.new && mv ~/.local/bin/agency.new ~/.local/bin/agency
launchctl kickstart -k gui/$(id -u)/local.agency.node
```

Linux では、最後の行の代わりにユニットを再起動します。`systemctl --user restart agency-node` です。
リリースビルドを使っているなら、新しい tarball を展開し、「リリースビルド」と同じ方法でバイナリを置いてから、ユニットを再起動します。
macOS では、`scripts/deploy-macos.sh` がこれらの手順を行い、最後に確認し、失敗したら元に戻します。

```sh
scripts/deploy-macos.sh --agency ~/.local/bin/agency --dir ~/.agency --label local.agency.node
```

- スクリプトのあるリポジトリから、Web クライアントとノードをビルドします。コミットしていない変更がある作業ツリーは断ります（`--allow-dirty` で許可できます）。はじめに、新しいバイナリが使い捨てのデータディレクトリで起動して応答することを確かめます（ポート 4790、または `SMOKE_PORT`。`--skip-preflight` でこの確認を省けます）。`--binary <ファイル>` を付けると、ビルドせずに、用意したバイナリをデプロイします。
- 新しいバイナリを 1 回の名前変更で置き換え、置き換えられた側は隣に `agency.previous` として残します。動いているノードは、再起動するまで、起動したときのファイルを使い続けます。
- ジョブを再起動し、新しいプロセスが `listen` のアドレス（別の場所は `--url` で指定）で応答するのを、最長 60 秒（`--timeout`）待ちます。そのプロセスがさらに 5 秒（`--settle`）動き続けることも確かめます。起動してすぐ止まるノードを、正常と取り違えないためです。
- 応答しなければ、前のバイナリを戻し、ジョブをもう一度再起動して、もう一度確かめます。終了コードで結果がわかります。0 はデプロイ成功、1 は未デプロイで、前のバイナリが戻って応答している状態、2 は未デプロイで、前のバイナリも応答しない状態です（すぐノードを見てください）。
- `--rollback` は、置いてあるバイナリと前のバイナリを入れ替え、同じように再起動して確かめます。2 回実行すると元に戻ります。

ジョブは先に読み込んでおく必要があります（「launchd で動かす」を参照）。スクリプトはデータディレクトリにもジョブのファイルにも触れません。同時に動かせるデプロイは 1 つだけです。

データディレクトリには手を加えません。
ノードは起動のたびに書き込み用のトークンを作り直すため、開いていたページは次の書き込みの前に一度読み込み直します。

### ログ

ノードは、動いているあいだに伝えるべきことを `~/.agency/logs/node.jsonl` に、1 行に 1 つの JSON オブジェクトとして書きます。

```json
{"ts":"2026-11-26T04:55:22.123Z","level":"warn","event":"runtime","message":"team acme/app: the team runtime fails: …"}
```

- `ts` は UTC です。`level` は `error`、`warn`、`info`、`debug` のどれかで、`logging.level` で変えない限り、ノードは `info` とそれより緊急なものを書きます。
- `event` は行を書いた部分の名前で、`start`、`runtime`、`refused`、`access`、`usage`、`push`、`placement`、`replicas`、`requests`、`outbox` のどれかです。`message` はノードについての文で、リクエストの本文、トークン、クエリ文字列は入りません。
- ファイルが `maxSizeMib` を超えると、ノードはそれを `node-<UTC の時刻>.jsonl` として閉じ、新しいファイルを始めます。閉じたファイルのうち `maxAgeDays` 日より古いものと、新しい `maxFiles` 個より前のものは、ノードの起動時と、ファイルを閉じるたびに削除されます。ファイルを読めるのはノードのアカウントだけです。
- ターミナルで動かすと、ノードは各行をそこにも表示します。ファイルに書けないときは、行は標準エラー出力へ行きます。launchd や systemd の下では、それが `serve.log` です。

`agency logs` は、閉じたファイルも含めてファイルを読み、ノードが止まっているときも、アカウントの鍵なしで、古い順に行を表示します。

```sh
agency logs --since 1h --level warn          # 直近 1 時間の warn と error
agency logs --event runtime                  # 1 つの部分だけ
agency logs --json | jq 'select(.event == "push")'   # ファイルにあるままの行
```

`--since` は整数と `s`、`m`、`h`、`d` のどれか（`90s`、`30m`、`2h`、`1d`）です。`--level` は、そのレベルとそれより緊急なものを残します。`--event` は 1 つの event を残します。ノードが書いている途中でクラッシュしたファイルの最後の行のように、1 行として完全でない行は飛ばします。

`config.json` に書いて、ノードを再起動します。どのフィールドも省けます。範囲外の値があると、ノードは起動時に止まり、どれかを伝えます。

```json
"logging": { "level": "info", "maxSizeMib": 20, "maxAgeDays": 14, "maxFiles": 50 }
```

| フィールド | 意味 | 値 | 既定値 |
|---|---|---|---|
| `level` | 書く行のうち、いちばん緊急でないレベル | `error`、`warn`、`info`、`debug` | `info` |
| `maxSizeMib` | ファイルを閉じるサイズ（MiB） | 1 から 1024 | 20 |
| `maxAgeDays` | 閉じたファイルを残す日数 | 1 から 365 | 14 |
| `maxFiles` | 残す閉じたファイルの数 | 1 から 1000 | 50 |

`serve.log` には、launchd や systemd が受け取るものが残ります。ノードの起動時に表示される行、ノードを止める失敗の行、クラッシュです。小さく、ローテーションはされません。

## トラブルシューティング

### ログの行

ノードは `~/.agency/logs/node.jsonl` に次の行を書きます。表は各行を、ターミナルと `serve.log` が表示する `agency: <message>` の形で示します。`node.jsonl` では、`agency: ` のあとの文が、その行の `message` です。`cannot listen` と、ノードを止める失敗の行は、`serve.log` にだけあります。

| 行 | 意味 |
|---|---|
| `Node running: <url> (listening on <address>)` と `Web client: …` | ノードが起動しました。 |
| `agency: cannot listen on 127.0.0.1:4400: Address already in use` | ほかのプロセスがポートを使っています。launchd と systemd は 10 秒ごとに起動し直します。`lsof -nP -iTCP:4400 -sTCP:LISTEN` でどのプロセスかわかります。 |
| `agency: team acme/app runs on this node (…), but the org places it at …` | ノードの `url` が変わっています。行に書かれた `agency team-move` を実行します。 |
| `agency: the access setting is malformed (…)` | `config.json` の `access` を直します。直すまで、Cloudflare を通ったリクエストはすべて拒否されます。 |
| `agency: the access appId is malformed (…)` | `appId` を直します。`access` のほかの部分は有効ですが、アプリのサインインは使えません。 |
| `agency: refused an Access request (no Access token)` | Access のサインインがないリクエストが Cloudflare から届きました。Bypass が二つのパス以外にも及んでいるか、ホスト名の前に Access がありません。 |
| `agency: refused an Access request (wrong audience)` | `audience` がアプリケーションの AUD タグと違います。 |
| `agency: refused an Access request (wrong issuer)` | `teamDomain` が自分のチームドメインと違います。 |
| `agency: refused an Access request (expired)` または `(not valid yet)` | サインインの期限が切れたか、このマシンの時計がずれています。 |
| `agency: refused an Access request (cannot fetch the Access certs: …)`、`(unknown signing key)`、`(the signature does not match)` | ノードが `teamDomain` から Access の鍵を取得できないか、トークンがこのチームドメインのものではありません。 |
| `agency: team acme/app: the team runtime fails: …` | チームランタイムが動かないか、5xx を返しました。行の残りに理由があります（下の応答の表を参照）。失敗し始めたときに 1 回書かれ、そのあとは理由が変わったときだけ書かれます。 |
| `agency: team acme/app: the team runtime answers again` | 失敗していたランタイムが、また応答するようになりました。 |
| `agency: refused GET /fed/v1/…: …` | ほかのノードからのリクエストを拒否しました。行に理由が書かれます（たとえば「a read signed by bob at …, 400 seconds behind this node's clock; check the clocks」）。相手には「not found」としか返していません。この種の行は 1 分に 20 行までで、それを超えた分は件数だけを 1 行で書きます。メンバーの正しい署名がある要求の拒否は、必ず書かれます。 |
| `agency: push notifications are off: …` | `push` のキーを読めません。通知は送られません。 |
| `agency: a push notification was not sent: …` | APNs が通知を受け付けませんでした。30 秒後にもう一度送ります。 |
| `agency: usage of "<id>" could not be read: …; trying again in <n> minutes` | その資金源の収集に失敗しました。理由が続きます（ログインが見つからない、トークンの期限切れ、トークンの拒否、サービスの不調）。最後の使用量ファイルは残ります。トークンが期限切れのときは、ノードが `claude` を実行して更新します（「使用量を読む」を参照）。理由がログインの拒否なら、その資金源の設定ディレクトリでログインし直すと、次の読み取りは成功します。続いて `agency: usage of "<id>" is read again` が出ます。 |
| `agency: re-reading the replicas failed: …`、`agency: checking for new requests failed: …`、`agency: retrying the outbox failed: …` | ノードがデータディレクトリに書き込めませんでした（ディスクがいっぱいなど）。ほかのノードに届かないことはログに出ません。送信待ちのものは `agency status` に、ほかのノードにあるチームの写しをいつ読んだかは Web クライアントに表示されます。 |
| `agency: team acme/app: a command the team runtime runs was not found: …` | チームランタイムが実行するコマンド（`gh` や `node` など）が、launchd のエージェントや systemd のユニットの `PATH` にないため、ランタイムが失敗しました。ターミナルで `command -v` を使って確かめ、それらが動くターミナルから「launchd で動かす」または「systemd で動かす」のスクリプトを実行し直して、再起動します。この行は `the team runtime fails: … ENOENT` の行に続いて出ます。 |
| `agency: <メッセージ>` のあとノードが止まる | `config.json` を読めないなどの理由で起動できませんでした。launchd と systemd は 10 秒ごとに起動し直します。 |

### ノードの応答

ブラウザや `curl` で見えるノードの応答では、`error` に何が起きたかが書かれます。

| 応答 | 意味 |
|---|---|
| `503` `cannot start the team runtime: No such file or directory` | `config.json` に書かれたパス（`teams[].runtime.command`）にチームランタイムのコマンドがありません。 |
| `503` `the team runtime did not answer within 20 seconds` | チームランタイムが止まっているか、応答が遅すぎます。 |
| `503` `the team runtime did not write an agency-runtime/1 response (…): …` | チームランタイムが失敗しました。応答の末尾は、チームランタイムが標準エラーに書いた内容です。launchd のエージェントや systemd のユニットの `PATH` にツールがないことがよくある原因です。 |
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
- チームランタイムが呼ぶすべてのツール（scrum のハーネスなら `node` と `gh`）が、launchd のエージェントや systemd のユニットに書かれた `PATH` から見つかること。見つからないコマンドがあるとランタイムが止まり、ノードのログがそう伝えます。
- 自分あての依頼は、`to` を省くか `owner` にすること。自分のほかにメンバーがいない組織では、すべての依頼が自分あてになります。自分を待っている参加者は、`waitingOn` が `{ "kind": "person", "person": "owner" }` になります。

ノードは動いているあいだ、30 秒ごとと、ページがチームを読むたびに、チームランタイムにビューを問い合わせます。
そのため、`view` は軽い処理にしておく必要があります。
