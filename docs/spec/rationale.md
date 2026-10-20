# 設計判断と参考資料

[仕様索引](README.md)へ戻る。節番号は分割前から維持する。

M0以後の計画を含む。実装済みの範囲は[開発計画](../PLAN.md)を参照する。

<a id="section-16"></a>

## 16. 採用しなかった案と理由

| 案                         | 判断理由                                                                 |
| -------------------------- | ------------------------------------------------------------------------ |
| BNDM/ABNDM                 | 短い候補、両端固定、内部ワイルドカードを優先するため既定にしない         |
| 無条件の長さフィルタ       | Include/WildcardSpaceで任意に長いtextを誤って落とす。下限のみ検査する    |
| UTF-32の全面保存           | 密な記号IDを検討し、表の大きさとキャッシュ費用を抑える                   |
| 独自の配信パッチ           | base/delta/tombstoneとmanifestを使い、独自パッチの検証・互換維持を避ける |
| 派生EQや絶対offsetの保存   | 更新に伴う後続データの変化を避ける。ロード費用との比較は製品で計測する   |
| 個別文字列の大量JS変換     | 一括経路は連結UTF-16またはバイナリ索引を使い、境界呼び出しを減らす       |
| UIのdebounceやMLランキング | ライブラリの責務外。入力制御やorderは呼び出し側が与える                  |

Myers、SIMD等の任意エンジンは採否を性能計測で決める。予測だけで優劣を確定しない。

<a id="section-18"></a>

## 18. 参考資料

- [同梱上流の出典・版・ライセンス](../../vendor/upstream/README.md)
- [asearch.ts](https://github.com/teramotodaiki/asearch.ts)
- [node-asearch](https://github.com/shokai/node-asearch)
- [asearch-ruby](https://github.com/masui/asearch-ruby)（アルゴリズムの由来。コードは取り込まない）
- [実行可能な参照実装](../../tools/reference/README.md)
