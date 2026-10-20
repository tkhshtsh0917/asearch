export declare enum MatchMode {
    /**
     * クエリ文字列がパターン文字列に完全一致する状態を「曖昧度0で一致」とする
     * デフォルトの設定
     */
    Exact = 0,
    /**
     * クエリ文字列の一部にパターン文字列が含まれている状態を「曖昧度0で一致」とする
     */
    Include = 1,
    /**
     * 半角スペースをワイルドカードとして扱う
     * オリジナルの asearch の挙動
     */
    WildcardSpace = 2
}
export declare class Asearch {
    /**
     * Asearch で扱えるパターン文字列の最大長
     * << と >>> が扱える最大のビット長 - 1
     * (受理状態を表すために 1 小さくしている)
     */
    static MAXLENGTH: number;
    /**
     * どのように比較するかを表すモード
     * 再設定はできないので変更したい場合はインスタンスを作る
     */
    readonly matchMode: MatchMode;
    /**
     * 検索したい文字列（パターン）
     * 再設定はできないのでパターンごとにインスタンスを作る
     */
    readonly pattern: string;
    /**
     * 受理状態のビットマスク
     */
    private acceptPattern;
    /**
     * 状態遷移機械の初期状態を表す数値
     * 最も左のビットが 1 で他は 0 の Unsigned int
     */
    private initPattern;
    /**
     * 状態遷移機械をループさせるためのビットマスク。
     * MatchMode.Include では、
     * 最初の一文字目で常にループさせつつ、
     * 一度受理状態になったらそれをループさせる。
     * MatchMode.Wildcard では、
     * ワイルドカードの位置でループさせる。
     */
    private loopMask;
    /**
     * パターン文字列の出現位置を表すビットマスク
     */
    masks: {
        [char: string]: number;
    };
    constructor(pattern: string, mode?: MatchMode);
    /**
     * 任意のクエリ文字列とパターン文字列を比較する。曖昧度を指定できる
     * @param query 検索文字列
     * @param ambig 曖昧度 := パターン文字列とのレーベンシュタイン距離
     */
    match(query: string, ambig?: number): boolean;
    private verbose;
}
