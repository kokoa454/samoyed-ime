# Third-Party Notices

SamoyedIME は、第三者が提供する辞書データおよび関連データを利用しています。各データには、それぞれ異なるライセンス、著作権表示および利用条件が適用されます。SamoyedIME 自身のソースコードに適用されるライセンスが、これらのデータすべてに自動的に適用されるわけではありません。

ライセンス本文と元の通知は `THIRD_PARTY_LICENSES/` に収録しています。以下は、このプロジェクトで報告された辞書構成に基づく出典一覧です。

## 1. Mozc Open Source Dictionary Data

**Source:** <https://github.com/google/mozc/tree/master/src/data/dictionary_oss>

SamoyedIME は Mozc の `src/data/dictionary_oss/` に由来する辞書データを利用しています。Mozc の公式説明では、このディレクトリのライセンスは `Mixed` とされ、IPAdic、ICOT Free Software、沖縄辞書などの通知・条件を確認するよう案内されています。

- 元の説明およびライセンス条件: `THIRD_PARTY_LICENSES/MOZC-DICTIONARY-OSS-README.txt`
- 主な内容: IPAdic/ICOT Free Software の著作権表示、保証の否認、沖縄辞書の Public Domain に関する声明

## 2. Mozc `id.def`

**Source:** <https://github.com/google/mozc/blob/master/src/data/dictionary_oss/id.def>

`id.def` は UT 辞書の生成処理で文脈 ID を更新するために使用されます。`merge-ut-dictionaries` のライセンス一覧では、このファイルについて BSD-3-Clause と明記されています。

- License text: `THIRD_PARTY_LICENSES/BSD-3-Clause.txt`

注: `merge-ut-dictionaries` のライセンス一覧は、重複除去に使用する Mozc の `dictionary*.txt` についても BSD-3-Clause と記しています。一方、Mozc 自身の公式説明は `dictionary_oss/` 全体を `Mixed` としています。この通知では、Mozc の辞書データ自体については Mozc の元 README を保持し、`id.def` の BSD-3-Clause 表示を別に記録しています。

## 3. Mozc UT Dictionary Data

SamoyedIME は <https://github.com/utuhiro78/merge-ut-dictionaries> を用いて生成した `mozcdic-ut.txt` を追加辞書として利用しています。提示された `make.sh` の設定では、次の4辞書が有効です。

| 辞書 | 出典 | 通知・ライセンス |
|---|---|---|
| `mozcdic-ut-neologd` | <https://github.com/utuhiro78/mozcdic-ut-neologd> | `THIRD_PARTY_LICENSES/MOZCDIC-UT-NEOLOGD-LICENSE.txt` および元データの `MECAB-IPADIC-NEOLOGD-COPYING.txt` |
| `mozcdic-ut-personal-names` | <https://github.com/utuhiro78/mozcdic-ut-personal-names> | Apache License 2.0。通知は `THIRD_PARTY_LICENSES/MOZCDIC-UT-PERSONAL-NAMES-LICENSE.txt`、本文は `THIRD_PARTY_LICENSES/APACHE-2.0.txt` |
| `mozcdic-ut-place-names` | <https://github.com/utuhiro78/mozcdic-ut-place-names> | 上流プロジェクトは Public Domain と表示。`THIRD_PARTY_LICENSES/MOZCDIC-UT-PLACE-NAMES-LICENSE.txt` を参照 |
| `mozcdic-ut-sudachidict` | <https://github.com/utuhiro78/mozcdic-ut-sudachidict> | Apache License 2.0。通知は `THIRD_PARTY_LICENSES/MOZCDIC-UT-SUDACHIDICT-LICENSE.txt`、元の `LEGAL` は `THIRD_PARTY_LICENSES/SUDACHIDICT-LEGAL.txt` |

`APACHE-2.0.txt` には Apache License 2.0 の全文を収録しています。SudachiDict の `LEGAL` には、SudachiDict 自身の Apache 2.0 表示に加え、UniDic および NEologd 由来データに関する追加の著作権表示・条件も含まれるため、同ファイルを別途保持しています。

### NEologd の元データ

NEologd の元データに関する著作権表示、謝辞および利用条件は `THIRD_PARTY_LICENSES/MECAB-IPADIC-NEOLOGD-COPYING.txt` に収録しています。出典: <https://github.com/neologd/mecab-ipadic-NEologd/blob/master/COPYING>

### 地名辞書・日本郵便の郵便番号データ

`mozcdic-ut-place-names` は日本郵便の郵便番号データを元に作成されたものです。同プロジェクトは「郵便番号データに限っては日本郵便株式会社は著作権を主張しません」と記しています。元の通知および公式の郵便番号データ案内は `THIRD_PARTY_LICENSES/MOZCDIC-UT-PLACE-NAMES-LICENSE.txt` を参照してください。

## 4. 辞書統合ツール

SamoyedIME は、[`merge-ut-dictionaries`](https://github.com/utuhiro78/merge-ut-dictionaries) を使用して生成した `mozcdic-ut.txt` を追加辞書として利用しています。

生成には、次の4種類の辞書データを使用しています。

- [mozcdic-ut-neologd](https://github.com/utuhiro78/mozcdic-ut-neologd)
- [mozcdic-ut-personal-names](https://github.com/utuhiro78/mozcdic-ut-personal-names)
- [mozcdic-ut-place-names](https://github.com/utuhiro78/mozcdic-ut-place-names)
- [mozcdic-ut-sudachidict](https://github.com/utuhiro78/mozcdic-ut-sudachidict)

辞書の生成には、Mozc の公開辞書データを重複エントリの除去に、`id.def` を文脈 ID の更新に使用しています。これらの出典と利用条件については、本書の該当するセクションを参照してください。

本プロジェクトの辞書生成処理では、Wikipedia の記事名インデックスを利用した単語コスト調整を無効にしています。Wikipedia 由来の追加辞書も使用していません。

生成された `mozcdic-ut.txt` は複数の辞書データを統合したものです。生成物全体に単一のライセンスが適用されると断定せず、各入力データに適用されるライセンスおよび利用条件に従って取り扱います。

**辞書統合ツール:** https://github.com/utuhiro78/merge-ut-dictionaries

**ソースコードのライセンス:** Apache License 2.0

ライセンス本文および元データの著作権表示・利用条件は、`THIRD_PARTY_LICENSES/` に収録しています。

## 5. 同梱文書

このパッケージには次の文書が含まれます。

- `THIRD_PARTY_LICENSES/APACHE-2.0.txt` — Apache License 2.0 全文
- `THIRD_PARTY_LICENSES/BSD-3-Clause.txt` — BSD 3-Clause License 本文（Mozc `id.def` に関する上流表示に基づく）
- `THIRD_PARTY_LICENSES/MOZC-DICTIONARY-OSS-README.txt` — Mozc 公開辞書の README と IPAdic/ICOT/沖縄辞書の通知
- `THIRD_PARTY_LICENSES/MECAB-IPADIC-NEOLOGD-COPYING.txt` — NEologd の元データ通知
- `THIRD_PARTY_LICENSES/MOZCDIC-UT-NEOLOGD-LICENSE.txt` — NEologd UT 辞書のライセンス参照
- `THIRD_PARTY_LICENSES/MOZCDIC-UT-PERSONAL-NAMES-LICENSE.txt` — 人名 UT 辞書のライセンス参照
- `THIRD_PARTY_LICENSES/MOZCDIC-UT-PLACE-NAMES-LICENSE.txt` — 地名 UT 辞書の Public Domain 通知
- `THIRD_PARTY_LICENSES/MOZCDIC-UT-SUDACHIDICT-LICENSE.txt` — SudachiDict UT 辞書のライセンス参照
- `THIRD_PARTY_LICENSES/SUDACHIDICT-LEGAL.txt` — SudachiDict の元の `LEGAL` 通知
- `THIRD_PARTY_LICENSES/MERGE-UT-DICTIONARIES-LICENSE.txt` — UT 辞書統合ツールのライセンス一覧

この通知は出典およびライセンス条件を整理するもので、個別ビルドの完全な入力履歴や、複合辞書に適用される法的結論を代替するものではありません。
