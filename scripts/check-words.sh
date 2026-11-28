#!/usr/bin/env bash
# Fails when a screen's text uses a word that docs/glossary.md says not to use and that a machine can tell
# apart from a good use: a Japanese or English word with one meaning only. Words that depend on what is
# meant (人 for a participant, 稟議書 for a document) are left to review. The glossary's "Do not use" table
# is the source of the list.
#
# Checked: the screens' text (web/src/i18n) and any Japanese document (docs/*.ja.md). Comment lines are not.
set -euo pipefail
cd "$(dirname "$0")/.."
shopt -s nullglob

# Japanese. ホスト is allowed only as part of ホスト名 (the public host name), and in two phrases that are not
# about a node or a machine of ours: セルフホスト (Cloudflare's "self-hosted" application type) and
# Apple のサーバー (Apple's own servers).
JAPANESE='拠点|コンピュータ|サーバ|ユーザ|ホスト|デバイス|判断のお願い'
# The names the glossary replaced (docs/glossary.md, "Do not use"): 参加者 is メンバー, エージェント is AI エージェント, リード is リーダー,
# and so on. リードタイム, AI エージェント, and 席 in 離席 and 席を外す (being away) are other words, so they are blanked out first.
JAPANESE="$JAPANESE|参加者|閲覧者|持ち主|オーナー|ルート鍵|回復用の|席|申請する人|外部の相談先|代理で答える|依頼|判断待ち|送り先|計画|週の予算|予算の状況|応答時間の予算|main が赤|全体の状況|成果の指標|エージェント|リード"
# English: a computer is a machine, a server is a node, a user is an account, a human, or a member.
ENGLISH='(^|[^A-Za-z])(computer|server|user|host)s?([^A-Za-z]|$)'

failed=0

# Prints the file's lines with comments and the allowed phrases blanked out, so line numbers still match.
visible() {
  sed -E 's|^[[:space:]]*//.*$||; s/ホスト名//g; s/セルフホスト//g; s/Apple のサーバー//g; s/リードタイム//g; s/AI エージェント//g; s/離席//g; s/席を外//g' "$1"
}

check() {
  local file="$1" pattern="$2" label="$3"
  if visible "$file" | grep -nEi -- "$pattern" | sed "s|^|$file:|"; then
    echo "  ^ $label: see docs/glossary.md, \"Do not use\"." >&2
    failed=1
  fi
}

for file in web/src/i18n/ja/*.ts docs/*.ja.md; do
  [ -f "$file" ] && check "$file" "$JAPANESE" "a word the glossary does not allow"
done
for file in web/src/i18n/en/*.ts; do
  [ -f "$file" ] && check "$file" "$ENGLISH" "a word the glossary does not allow"
done

exit "$failed"
