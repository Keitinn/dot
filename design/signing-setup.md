# 署名・公証の設定手順(Issue #9)

dot を Developer ID で署名し、Apple の公証を受けて、普通にインストールできるようにする手順です。
**コードと CI の側は用意済み**です(`release.yml`)。下の「オーナーが行う作業」が済むまでは、これまでどおり署名なし(ad-hoc)のビルドになります。

## 決定済み
- バンドル ID: `io.github.keitinn.dot`(`backend/tauri.conf.json`)
- Apple 側の App ID: `io.github.keitinn.dot`(ID `HSP5Y2WN6T`、チーム `X8UC7936LR`)。iCloud capability 有効化済み

## オーナーが行う作業(Apple ID のログインや Account Holder の権限が要るため、自動化できない)
1. **Developer ID Application 証明書を作る**
   - Xcode → 設定 → Accounts → チームを選択 → Manage Certificates → 「+」→ Developer ID Application
   - 理由: App Store Connect の API キーでは「Account Holder のみ」と拒否された
2. **証明書を .p12 に書き出す**
   - キーチェーンアクセス → 「Developer ID Application: …」を右クリック → 書き出す → `.p12`(パスワードを設定)
   - パスワードはチャットに書かない
3. **GitHub の Secrets に登録する**(リポジトリ Keitinn/dot → Settings → Secrets and variables → Actions)
   - `APPLE_CERTIFICATE`: .p12 を base64 にした文字列(`base64 -i cert.p12 | pbcopy`)
   - `APPLE_CERTIFICATE_PASSWORD`: .p12 のパスワード
   - `APPLE_SIGNING_IDENTITY`: `Developer ID Application: 名前 (X8UC7936LR)`(`security find-identity -v -p codesigning` で確認)
4. **公証用の App Store Connect API キーを用意する**
   - App Store Connect → ユーザとアクセス → 統合 → キー。公証には、`hermes` とは別に、専用のキーを作る方が安全(ロールは Developer で足りる)
   - Secrets に登録: `APPLE_API_ISSUER`(Issuer ID)、`APPLE_API_KEY`(Key ID)、`APPLE_API_KEY_P8`(.p8 ファイルの中身そのまま)
5. **(CloudKit を使う段階で)iCloud コンテナを作る**
   - ターミナルで `asc web auth login`(Apple ID のパスワードと 2FA は本人が入力)→ `asc web icloud-containers create --identifier iCloud.io.github.keitinn.dot --name dot --confirm`
   - **作ると消せない**。署名・公証だけなら不要

## 私(Hermes)が行う作業
- 手順 1〜4 が済んだら、`v1.2.1` のタグで署名つきのビルドを試す(タグの push は指示を受けてから)
- CI の署名検証(`codesign --verify`、`spctl --assess`、`stapler validate`)が通ることを確認
- 手元のダウンロードで、隔離属性を付けた状態から、警告なしで開けることを確認

## 署名で変わること(影響)
- **バンドル ID の変更**: macOS は別のアプリとして扱う。以前のバージョンに許可した項目(ログイン項目など)は引き継がれない。**セーブデータは影響なし**(`~/Library/Application Support/dot`、バンドル ID に依存しない)
- README の `xattr -cr` の手順は、署名後は不要になる
- Homebrew cask `dot` は upstream(tooyipjee)のもの。このフォークのリリースを指さない

## 未確認(実際にやってみないと分からない)
- CI 上の署名と公証が一度で通るか(Secrets が無いと試せないため、未実行)
- Tauri の `APPLE_API_KEY_PATH` を `$GITHUB_ENV` 経由で渡す方法が、ビルドステップに届くか
- x86_64 と aarch64 の両方で公証が通る時間(各数分〜)
- CloudKit 用の entitlements と provisioning profile の扱い(段階2で別途設計)
