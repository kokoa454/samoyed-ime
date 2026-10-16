use crate::dictionary::{DictEntry, Dictionary};


/// 単語間の連接コストを提供する。
///
/// start_cost:
/// 文頭から最初の単語へ接続するコスト。
///
/// transition_cost:
/// 前の単語のright_idから、次の単語のleft_idへ接続するコスト。
///
/// end_cost:
/// 最後の単語から文末へ接続するコスト。
///
/// Noneは、その接続が利用できないことを表す。
/// 実際のMozcデータを使う場合、文頭・文末の扱いは
/// データ形式を確認したうえで実装する。
pub trait ConnectionCostProvider {
    fn start_cost(&self, left_id: u16) -> Option<i32>;

    fn transition_cost(
        &self,
        previous_right_id: u16,
        next_left_id: u16,
    ) -> Option<i32>;

    fn end_cost(&self, right_id: u16) -> Option<i32>;
}

/// かな漢字変換の候補
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct KanjiCandidate {
    pub surface: String, // 表層形
    pub left_id: u16, // 左ID
    pub right_id: u16, // 右ID
    pub word_cost: i32, // 単語コスト
    pub contextual_cost: i64, // 文脈コスト
}

/// 変換結果の1文節
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ConversionSegment {
    pub reading: String, // 読み
    pub start: usize, // 開始位置
    pub end: usize, // 終了位置
    pub candidates: Vec<KanjiCandidate>, // 変換候補
    pub selected_index: usize, // 選択中の候補インデックス
}

impl ConversionSegment {
    /// 選択中の候補を取得する。
    ///
    /// 戻り値:
    /// * `Some(&KanjiCandidate)`: 選択中の候補
    /// * `None`: 選択中の候補が存在しない
    pub fn selected_candidate(&self) -> Option<&KanjiCandidate> {
        self.candidates.get(self.selected_index)
    }
}

/// かな漢字変換の結果
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct KanjiConversionResult {
    pub reading: String, // 読み
    pub segments: Vec<ConversionSegment>, // 文節
    pub total_cost: i64, // 累積コスト
}

impl KanjiConversionResult {
    /// 各文節の選択中候補を連結した文字列を取得する。
    ///
    /// 戻り値:
    /// * `String`: 各文節の選択中候補を連結した文字列
    pub fn surface(&self) -> String {
        let mut output = String::new();

        for segment in &self.segments {
            if let Some(candidate) = segment.selected_candidate() {
                output.push_str(&candidate.surface);
            }
        }

        output
    }
}

/// かな漢字変換のエラー
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum KanjiConversionError {
    NoPath, // 辞書経路不明
}

/// ラティス上の単語候補
#[derive(Debug, Clone)]
struct LatticeNode {
    start: usize, // 開始位置（文字数）
    end: usize, // 終了位置（文字数）
    reading: String, // 読み
    entry: DictEntry, // 辞書エントリ
    best_prefix_cost: Option<i64>, // 文頭からこの候補までの最小コスト
    previous: Option<usize>, // 最小コスト経路で直前に選ばれた候補のインデックス
    best_suffix_cost: Option<i64>, // この候補の後ろから文末までの最小コスト
}

/// かな漢字変換器
pub struct KanjiConverter<'a, C> {
    dictionary: &'a Dictionary, // 辞書
    connection_costs: &'a C, // 连接コスト
}

impl<'a, C: ConnectionCostProvider> KanjiConverter<'a, C> {
    /// コンストラクタ。
    pub fn new(
        dictionary: &'a Dictionary,
        connection_costs: &'a C,
    ) -> Self {
        Self {
            dictionary,
            connection_costs,
        }
    }

    /// 読みをかな漢字変換する。
    ///
    /// 引数
    /// * `reading`: 変換する文字列
    ///
    /// 戻り値
    /// * `KanjiConversionResult`: 変換結果
    /// * `KanjiConversionError::NoPath`: 変換結果が存在しない
    pub fn convert(
        &self,
        reading: &str,
    ) -> Result<KanjiConversionResult, KanjiConversionError> {
        if reading.is_empty() {
            return Ok(KanjiConversionResult {
                reading: String::new(),
                segments: Vec::new(),
                total_cost: 0,
            });
        }

        // 文字位置とUTF-8バイト位置の対応を作る
        let byte_offsets: Vec<usize> = reading
            .char_indices()
            .map(|(byte_index, _)| byte_index)
            .chain(std::iter::once(reading.len()))
            .collect();

        let char_count = byte_offsets.len() - 1;

        // 各候補をラティスのノードとして登録する
        let mut nodes = Vec::<LatticeNode>::new();
        let mut nodes_by_start = vec![Vec::<usize>::new(); char_count + 1];
        let mut nodes_by_end = vec![Vec::<usize>::new(); char_count + 1];

        for start in 0..char_count {
            let suffix = &reading[byte_offsets[start]..];

            for prefix_match in self.dictionary.common_prefix_search(suffix) {
                let matched_length = prefix_match.reading.chars().count();

                if matched_length == 0 {
                    continue;
                }

                let end = start + matched_length;

                if end > char_count {
                    continue;
                }

                for entry in prefix_match.candidates {
                    let node_index = nodes.len();

                    nodes.push(LatticeNode {
                        start,
                        end,
                        reading: prefix_match.reading.to_string(),
                        entry: entry.clone(),
                        best_prefix_cost: None,
                        previous: None,
                        best_suffix_cost: None,
                    });

                    nodes_by_start[start].push(node_index);
                    nodes_by_end[end].push(node_index);
                }
            }
        }

        if nodes.is_empty() {
            return Err(KanjiConversionError::NoPath);
        }

        // 前向きViterbi探索
        // 各候補について、そこまでの最小累積コストを計算する
        // 単語の生起コストと、直前の単語からの連接コストを加算する
        for start in 0..char_count {
            for &node_index in &nodes_by_start[start] {
                let left_id = nodes[node_index].entry.left_id;
                let word_cost = i64::from(nodes[node_index].entry.cost);

                let mut best_cost: Option<i64> = None;
                let mut best_previous: Option<usize> = None;

                if start == 0 {
                    if let Some(start_cost) = self.connection_costs.start_cost(left_id) {
                        best_cost = i64::from(start_cost).checked_add(word_cost);
                    }
                } else {
                    for &previous_index in &nodes_by_end[start] {
                        let Some(previous_cost) =
                            nodes[previous_index].best_prefix_cost
                        else {
                            continue;
                        };

                        let previous_right_id =
                            nodes[previous_index].entry.right_id;

                        let Some(connection_cost) = self
                            .connection_costs
                            .transition_cost(previous_right_id, left_id)
                        else {
                            continue;
                        };

                        let Some(candidate_cost) = previous_cost
                            .checked_add(i64::from(connection_cost))
                            .and_then(|cost| cost.checked_add(word_cost))
                        else {
                            continue;
                        };

                        if best_cost.map_or(true, |cost| candidate_cost < cost) {
                            best_cost = Some(candidate_cost);
                            best_previous = Some(previous_index);
                        }
                    }
                }

                nodes[node_index].best_prefix_cost = best_cost;
                nodes[node_index].previous = best_previous;
            }
        }

        // 後ろ向きの最小コスト計算
        // 各候補から文末までの最小コストを計算する
        // この情報は、同じ文節範囲にある候補の文脈込みランキングに使う
        for start in (0..char_count).rev() {
            for &node_index in &nodes_by_start[start] {
                let end = nodes[node_index].end;
                let right_id = nodes[node_index].entry.right_id;

                let mut best_cost: Option<i64> = None;

                if end == char_count {
                    best_cost = self
                        .connection_costs
                        .end_cost(right_id)
                        .map(i64::from);
                } else {
                    for &next_index in &nodes_by_start[end] {
                        let Some(suffix_cost) =
                            nodes[next_index].best_suffix_cost
                        else {
                            continue;
                        };

                        let next_left_id = nodes[next_index].entry.left_id;
                        let next_word_cost =
                            i64::from(nodes[next_index].entry.cost);

                        let Some(connection_cost) = self
                            .connection_costs
                            .transition_cost(right_id, next_left_id)
                        else {
                            continue;
                        };

                        let Some(candidate_cost) = i64::from(connection_cost)
                            .checked_add(next_word_cost)
                            .and_then(|cost| cost.checked_add(suffix_cost))
                        else {
                            continue;
                        };

                        if best_cost.map_or(true, |cost| candidate_cost < cost) {
                            best_cost = Some(candidate_cost);
                        }
                    }
                }

                nodes[node_index].best_suffix_cost = best_cost;
            }
        }

        // 文末まで到達できる最良経路を選択する
        let mut best_final: Option<(usize, i64)> = None;

        for &node_index in &nodes_by_end[char_count] {
            let (Some(prefix_cost), Some(suffix_cost)) = (
                nodes[node_index].best_prefix_cost,
                nodes[node_index].best_suffix_cost,
            ) else {
                continue;
            };

            let Some(total_cost) = prefix_cost.checked_add(suffix_cost) else {
                continue;
            };

            if best_final.map_or(true, |(_, cost)| total_cost < cost) {
                best_final = Some((node_index, total_cost));
            }
        }

        let Some((best_end, total_cost)) = best_final else {
            return Err(KanjiConversionError::NoPath);
        };

        // 最良経路を逆向きにたどり、文節列を復元する
        let mut path = Vec::new();
        let mut current = best_end;

        loop {
            path.push(current);

            let Some(previous) = nodes[current].previous else {
                break;
            };

            current = previous;
        }

        path.reverse();

        // 選択経路の各文節について、同じ読み範囲の候補を集める
        // best_prefix_cost + best_suffix_costで、候補を含む経路の最小コストを計算し、文脈を考慮した候補順位を付ける
        let mut segments = Vec::with_capacity(path.len());

        for &selected_node_index in &path {
            let selected_node = &nodes[selected_node_index];
            let start = selected_node.start;
            let end = selected_node.end;

            let mut candidates: Vec<(usize, KanjiCandidate)> = Vec::new();

            for &candidate_node_index in &nodes_by_start[start] {
                let candidate_node = &nodes[candidate_node_index];

                if candidate_node.end != end {
                    continue;
                }

                let (Some(prefix_cost), Some(suffix_cost)) = (
                    candidate_node.best_prefix_cost,
                    candidate_node.best_suffix_cost,
                ) else {
                    continue;
                };

                let Some(contextual_cost) = prefix_cost.checked_add(suffix_cost)
                else {
                    continue;
                };

                candidates.push((
                    candidate_node_index,
                    KanjiCandidate {
                        surface: candidate_node.entry.surface.clone(),
                        left_id: candidate_node.entry.left_id,
                        right_id: candidate_node.entry.right_id,
                        word_cost: candidate_node.entry.cost,
                        contextual_cost,
                    },
                ));
            }

            candidates.sort_by(|(left_index, left), (right_index, right)| {
                left.contextual_cost
                    .cmp(&right.contextual_cost)
                    .then_with(|| left.word_cost.cmp(&right.word_cost))
                    .then_with(|| left.surface.cmp(&right.surface))
                    .then_with(|| left_index.cmp(right_index))
            });

            let selected_index = candidates
                .iter()
                .position(|(node_index, _)| {
                    *node_index == selected_node_index
                })
                .ok_or(KanjiConversionError::NoPath)?;

            segments.push(ConversionSegment {
                reading: selected_node.reading.clone(),
                start,
                end,
                candidates: candidates
                    .into_iter()
                    .map(|(_, candidate)| candidate)
                    .collect(),
                selected_index,
            });
        }

        Ok(KanjiConversionResult {
            reading: reading.to_string(),
            segments,
            total_cost,
        })
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::collections::{HashMap, HashSet};

    /// テスト用の接続コスト。
    /// 実際のMozcの接続コストではない。
    #[derive(Default)]
    struct TestConnectionCosts {
        transitions: HashMap<(u16, u16), i32>,
        start_costs: HashMap<u16, i32>,
        end_costs: HashMap<u16, i32>,
        unavailable_starts: HashSet<u16>,
        unavailable_transitions: HashSet<(u16, u16)>,
        unavailable_ends: HashSet<u16>,
    }

    impl ConnectionCostProvider for TestConnectionCosts {
        fn start_cost(&self, left_id: u16) -> Option<i32> {
            if self.unavailable_starts.contains(&left_id) {
                return None;
            }

            Some(*self.start_costs.get(&left_id).unwrap_or(&0))
        }

        fn transition_cost(
            &self,
            previous_right_id: u16,
            next_left_id: u16,
        ) -> Option<i32> {
            let key = (previous_right_id, next_left_id);

            if self.unavailable_transitions.contains(&key) {
                return None;
            }

            Some(*self.transitions.get(&key).unwrap_or(&0))
        }

        fn end_cost(&self, right_id: u16) -> Option<i32> {
            if self.unavailable_ends.contains(&right_id) {
                return None;
            }

            Some(*self.end_costs.get(&right_id).unwrap_or(&0))
        }
    }

    fn create_dictionary(text: &str) -> Dictionary {
        let mut dictionary = Dictionary::new();
        dictionary.load_tsv(text);
        dictionary.finish();
        dictionary
    }

    #[test]
    fn viterbi_selects_path_using_transition_costs() {
        let dictionary = create_dictionary(
            "あ\t1\t1\t0\t亜\n\
             あい\t2\t2\t100\t愛\n\
             い\t3\t3\t0\t胃\n\
             う\t4\t4\t0\t鵜\n\
             いう\t5\t5\t200\t言う\n",
        );

        let mut costs = TestConnectionCosts::default();

        // 「あ」→「い」の接続を高コストにする。
        costs.transitions.insert((1, 3), 1000);

        let converter = KanjiConverter::new(&dictionary, &costs);
        let result = converter.convert("あいう").unwrap();

        let readings: Vec<&str> = result
            .segments
            .iter()
            .map(|segment| segment.reading.as_str())
            .collect();

        assert_eq!(readings, vec!["あい", "う"]);
        assert_eq!(result.surface(), "愛鵜");
        assert_eq!(result.total_cost, 100);
    }

    #[test]
    fn candidate_ranking_uses_surrounding_context() {
        let dictionary = create_dictionary(
            "あい\t2\t2\t10\t優\n\
             あい\t6\t6\t0\t安\n\
             う\t4\t4\t0\t兎\n",
        );

        let mut costs = TestConnectionCosts::default();

        // 「安」は単語コストが低いが、後ろの「兎」との接続を高くする。
        costs.transitions.insert((6, 4), 100);

        let converter = KanjiConverter::new(&dictionary, &costs);
        let result = converter.convert("あいう").unwrap();

        let first_segment = &result.segments[0];

        assert_eq!(first_segment.reading, "あい");
        assert_eq!(
            first_segment.selected_candidate().unwrap().surface,
            "優"
        );

        let better_context_cost = first_segment
            .candidates
            .iter()
            .find(|candidate| candidate.surface == "優")
            .unwrap()
            .contextual_cost;

        let worse_context_cost = first_segment
            .candidates
            .iter()
            .find(|candidate| candidate.surface == "安")
            .unwrap()
            .contextual_cost;

        assert!(better_context_cost < worse_context_cost);
    }

    #[test]
    fn conversion_fails_when_no_complete_path_exists() {
        let dictionary = create_dictionary("あ\t1\t1\t0\t亜\n");
        let costs = TestConnectionCosts::default();

        let converter = KanjiConverter::new(&dictionary, &costs);

        assert_eq!(
            converter.convert("あい"),
            Err(KanjiConversionError::NoPath)
        );
    }

    #[test]
    fn empty_reading_returns_empty_result() {
        let dictionary = create_dictionary("");
        let costs = TestConnectionCosts::default();

        let converter = KanjiConverter::new(&dictionary, &costs);
        let result = converter.convert("").unwrap();

        assert!(result.segments.is_empty());
        assert_eq!(result.surface(), "");
        assert_eq!(result.total_cost, 0);
    }

    #[test]
    fn converts_with_embedded_mozc_dictionary_and_connection_costs() {
        let dictionary = Dictionary::embedded();

        let connection_costs =
            crate::connection_costs::ConnectionCosts::embedded()
                .expect("embedded connection costs must be valid");

        let converter =
            KanjiConverter::new(dictionary, &connection_costs);

        let result = converter
            .convert("にほんご")
            .expect("にほんご should have a complete conversion path");

        // 入力した読みが変換結果に保持されている。
        assert_eq!(result.reading, "にほんご");

        // 実際のMozc辞書から「日本語」を選択できることを確認する。
        assert_eq!(result.surface(), "日本語");

        // 文節の読みを連結すると、入力した読みと一致する。
        let joined_reading: String = result
            .segments
            .iter()
            .map(|segment| segment.reading.as_str())
            .collect();

        assert_eq!(joined_reading, "にほんご");

        // 文節の開始・終了位置が連続していることを確認する。
        assert_eq!(
            result.segments.first().map(|segment| segment.start),
            Some(0)
        );

        assert_eq!(
            result.segments.last().map(|segment| segment.end),
            Some("にほんご".chars().count())
        );

        for pair in result.segments.windows(2) {
            assert_eq!(pair[0].end, pair[1].start);
        }

        // 選択された候補から総コストを再計算する。
        let selected_candidates: Vec<&KanjiCandidate> = result
            .segments
            .iter()
            .map(|segment| {
                segment
                    .selected_candidate()
                    .expect("each segment must have a selected candidate")
            })
            .collect();

        let first = selected_candidates
            .first()
            .expect("conversion result must contain at least one segment");

        let mut recomputed_cost =
            i64::from(connection_costs.start_cost(first.left_id).unwrap());

        recomputed_cost += i64::from(first.word_cost);

        for pair in selected_candidates.windows(2) {
            let previous = pair[0];
            let next = pair[1];

            let transition_cost = connection_costs
                .transition_cost(previous.right_id, next.left_id)
                .expect("selected path must have valid transitions");

            recomputed_cost += i64::from(transition_cost);
            recomputed_cost += i64::from(next.word_cost);
        }

        let last = selected_candidates
            .last()
            .expect("conversion result must contain at least one segment");

        recomputed_cost += i64::from(
            connection_costs
                .end_cost(last.right_id)
                .expect("selected path must have a valid end connection"),
        );

        assert_eq!(
            result.total_cost,
            recomputed_cost,
            "reported total cost must match the selected segment path"
        );
    }

    #[test]
    fn start_cost_affects_selected_candidate() {
        let dictionary = create_dictionary(
            "あ\t1\t1\t0\t文頭コスト大\n\
             あ\t2\t2\t0\t文頭コスト小\n",
        );

        let mut costs = TestConnectionCosts::default();

        costs.start_costs.insert(1, 100);

        let converter = KanjiConverter::new(&dictionary, &costs);
        let result = converter.convert("あ").unwrap();

        assert_eq!(result.surface(), "文頭コスト小");
        assert_eq!(result.total_cost, 0);
    }

    #[test]
    fn end_cost_affects_selected_candidate() {
        let dictionary = create_dictionary(
            "あ\t1\t1\t0\t文末コスト大\n\
             あ\t2\t2\t0\t文末コスト小\n",
        );

        let mut costs = TestConnectionCosts::default();

        costs.end_costs.insert(1, 100);

        let converter = KanjiConverter::new(&dictionary, &costs);
        let result = converter.convert("あ").unwrap();

        assert_eq!(result.surface(), "文末コスト小");
        assert_eq!(result.total_cost, 0);
    }

    #[test]
    fn conversion_fails_when_start_connection_is_unavailable() {
        let dictionary = create_dictionary(
            "あ\t1\t1\t0\t亜\n",
        );

        let mut costs = TestConnectionCosts::default();
        costs.unavailable_starts.insert(1);

        let converter = KanjiConverter::new(&dictionary, &costs);

        assert_eq!(
            converter.convert("あ"),
            Err(KanjiConversionError::NoPath)
        );
    }

    #[test]
    fn conversion_fails_when_end_connection_is_unavailable() {
        let dictionary = create_dictionary(
            "あ\t1\t1\t0\t亜\n",
        );

        let mut costs = TestConnectionCosts::default();
        costs.unavailable_ends.insert(1);

        let converter = KanjiConverter::new(&dictionary, &costs);

        assert_eq!(
            converter.convert("あ"),
            Err(KanjiConversionError::NoPath)
        );
    }

    #[test]
    fn diagnose_ii_to_omou_yo_conversion() {
        let reading = "いいとおもうよ";

        let dictionary = Dictionary::embedded();
        let connection_costs =
            crate::connection_costs::ConnectionCosts::embedded().unwrap();

        // 入力の各位置から、辞書に一致する読みを列挙する。
        for (char_index, (byte_index, _)) in
            reading.char_indices().enumerate()
        {
            let suffix = &reading[byte_index..];

            let matches: Vec<_> = dictionary
                .common_prefix_search(suffix)
                .iter()
                .map(|item| {
                    let surfaces: Vec<String> = item
                        .candidates
                        .iter()
                        .map(|entry| entry.surface.clone())
                        .collect();

                    (item.reading.to_string(), surfaces)
                })
                .collect();

            println!(
                "位置={char_index}, 残りの読み={suffix:?}, 一致={matches:?}"
            );
        }

        // 入力全体を Viterbi 変換する。
        let converter =
            KanjiConverter::new(dictionary, &connection_costs);

        let result = converter.convert(reading);

        println!("Viterbi変換結果: {result:#?}");
    }
}
