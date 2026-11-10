# 作業方針

- セッションの記録に依存せず作業を継続できるよう、必要な情報を文書に残し、常に現状を反映する。
- コードと文書は情報や責務の正本を一意に保ち、不要な重複・複雑性・依存関係を避ける。
- 文書とディレクトリ構造は現状を示す。履歴はGitで管理し、現行設計に必要な経緯のみ文書に残す。
- コミットは論理的な区切りで適時作成し、無関係な変更を混在させない。
- `development/TODO.md` を未完了作業の正本とし、進捗と新たに発見した問題を反映する。
- ユーザーの指示や既存の設計を無条件に信用せず、目的・正確性・論理性を優先する。

# 作業開始時

以下を必ず実施する。

- 目的・要求・仕様・実装・`development/TODO.md` の現状を確認し、作業単位と完了条件を決定する。
- 関連する設計・責務分担・依存関係・ディレクトリ構造・作業手順を再評価し、より合理的な構成があれば改善する。

# 作業完了時

論理的な作業単位の完了時に、以下を必ず実施する。

- 変更したコードと関連箇所を、異なる観点から必ず2回以上レビューする。
  - 1回目：仕様・設計・実装の正確性と整合性。
  - 2回目：潜在的な欠陥・境界条件・重複・複雑性・設計改善の余地。
- 過去のレビュー結果を無条件に信用せず、問題の発見を優先する。発見した問題は修正し、影響範囲を再レビューする。
- 文書と実装の整合性を確認し、重複・不整合・不合理な構造を改善する。
- テストの前提・期待結果・網羅範囲を疑い、検証方法の問題を改善する。
- 新たに発見した問題は、重複を確認して `development/TODO.md` に `- [ ]` で追加する。
- 実装・レビュー・必要な検証・文書更新が完了した項目のみ `- [x]` に変更する。

# 品質基準

- 部分的な完成度より、成果物全体の品質と整合性を優先する。
- 変更量の少なさを優先せず、目的達成に必要な改善を省略しない。
- 正確性・単純性・保守性・検証可能性を重視し、根本原因の解決を優先する。
- 既存の実装やテストの成功を、正しさや設計の最適性の証明とみなさない。
- 検証は影響範囲とリスクに応じて実施し、小規模な変更のたびに高コストな全体検証を繰り返さない。

# 自律継続基準

- `development/TODO.md` に `- [ ]` が残る限り、ユーザーの次の指示を待たずに作業ループを継続する。
- 各作業で「作業開始時」と「作業完了時」の手順を必ず実施する。
- 次の作業は依存関係・重要性・影響範囲から選択する。
- 全項目の完了後は最終レビューを実施し、新たな問題が見つかれば `development/TODO.md` に追加して作業を継続する。
- 未完了項目がなく、最終レビューでも問題が見つからなければ正常終了する。
- ユーザーから停止指示があった場合、または権限・判断・実行環境などの制約で継続不能な場合は、作業状態と残作業を保存して停止する。

# Nocter Repository Rules

## Public Documentation Language

Write all public-facing documentation in English. This includes the repository `README.md`,
`spec/`, `development/spec/`, `development/README.md`, and `development/architecture/`, plus release notes and generated website
content.

Internal agent instructions and handoff files that are excluded from the public documentation build,
such as `AGENTS.md` and `development/TODO.md`, may use another language when it improves team
communication.

Edit the source Markdown rather than generated HTML. After changing public Markdown, run the
documentation generator tests and build into an explicit directory outside the repository. Do not
commit generated website output; GitHub Pages receives an artifact built from `main`.

## Documentation Placement

Keep public documentation authored at the repository root user-facing. The root `README.md` may
explain what Nocter is, how to install and use a released version, and where to find further
documentation, but it must not contain contributor setup, repository-local workflows,
implementation status, or milestone plans. Internal control files such as `AGENTS.md` are not
public documentation.

Keep all contributor and compiler-development documentation under `development/`. This includes
build and test instructions, repository-local packaging, architecture, implementation design,
milestone planning, maintenance policy, and handoff state. Root documentation may link to the
`development/` entry point, but must not duplicate its contents.

Published language, platform and tooling semantics belong under `spec/`. Adopted unreleased
contracts belong under `development/spec/`, which is excluded from website publication and search.
Each version has one normative owner per subject; proposals and superseded decisions remain in
`development/history/`. A public release promotes contracts by moving them, not retaining editable
copies in both trees. Adopted unreleased library changes belong under `development/spec/library/`.
Published standard-library declarations and declaration documentation belong to each root `std/**/index.nct`; `std/README.md` assigns each longer
observable subject to one behavior guide. The product sources and their API documentation remain
together under `std/`; contributor-only implementation contracts remain under `development/`.

Within `spec/`, keep source semantics under `language/`, target and ABI contracts under `platform/`,
and observable compiler/editor behavior under `tooling/`. File names describe concepts rather than
release order; the `README.md` catalogs recommend reading order but do not register pages.
Generated navigation follows the directory structure of every published source automatically.

Keep milestones, reviews, release qualification, and superseded design under
`development/history/`. Historical records are not published as current website guidance, but
milestone, review, and release-audit catalogs and local links must remain valid.

Keep complete runnable user packages under `examples/`. Do not place compiler diagnostic fixtures
or deliberately invalid source there. Compiler source-corpus inputs belong under
`development/compiler/tests/fixtures/`.

## Version Terminology

Use an exact release or milestone such as `v0.2.0` or `v0.3.0 Phase 0` in public documentation.
Do not use bare `v0` as a release name or compatibility boundary. Released contracts remain
versioned records; active design documents must distinguish adopted direction from implemented
behavior.
