//! 依存グラフ: 強連結成分と、依存先が先に来るトポロジカル順（SPEC 第 7 章、DECISIONS B9・C7）。
//!
//! 長い依存の鎖でもスタックを使い切らないよう、再帰を使わずに書く。

use std::collections::BTreeSet;

#[derive(Clone, Copy)]
struct State {
    index: Option<usize>,
    low: usize,
    on_stack: bool,
}

/// 強連結成分を求める（Tarjan の方法）。`adj[u]` は `u` が依存する（参照する）ノード。
/// 成分は、依存先の成分が先に来る順に並び、各成分の中は番号順。
pub(crate) fn sccs(adj: &[Vec<usize>]) -> Vec<Vec<usize>> {
    let n = adj.len();
    let mut state = vec![
        State {
            index: None,
            low: 0,
            on_stack: false,
        };
        n
    ];
    let mut stack: Vec<usize> = Vec::new();
    let mut next = 0usize;
    let mut out = Vec::new();
    let edges = |v: usize| adj.get(v).map_or(&[][..], Vec::as_slice);

    for root in 0..n {
        if state.get(root).is_none_or(|s| s.index.is_some()) {
            continue;
        }
        let visit = |v: usize, state: &mut Vec<State>, stack: &mut Vec<usize>, next: &mut usize| {
            if let Some(s) = state.get_mut(v) {
                *s = State {
                    index: Some(*next),
                    low: *next,
                    on_stack: true,
                };
            }
            *next = next.saturating_add(1);
            stack.push(v);
        };
        visit(root, &mut state, &mut stack, &mut next);
        let mut call: Vec<(usize, usize)> = vec![(root, 0)];
        while let Some(&(v, ei)) = call.last() {
            if let Some(&w) = edges(v).get(ei) {
                if let Some(top) = call.last_mut() {
                    top.1 = ei.saturating_add(1);
                }
                let Some(ws) = state.get(w).copied() else {
                    continue;
                };
                match ws.index {
                    None => {
                        visit(w, &mut state, &mut stack, &mut next);
                        call.push((w, 0));
                    }
                    Some(wi) if ws.on_stack => {
                        if let Some(s) = state.get_mut(v) {
                            s.low = s.low.min(wi);
                        }
                    }
                    Some(_) => {}
                }
            } else {
                call.pop();
                let Some(vs) = state.get(v).copied() else {
                    continue;
                };
                if let Some(&(u, _)) = call.last()
                    && let Some(s) = state.get_mut(u)
                {
                    s.low = s.low.min(vs.low);
                }
                if Some(vs.low) == vs.index {
                    let mut component = Vec::new();
                    while let Some(w) = stack.pop() {
                        if let Some(s) = state.get_mut(w) {
                            s.on_stack = false;
                        }
                        component.push(w);
                        if w == v {
                            break;
                        }
                    }
                    component.sort_unstable();
                    out.push(component);
                }
            }
        }
    }
    out
}

/// 循環しているか（2 つ以上のノードの成分か、自分自身への辺がある）。
pub(crate) fn is_cycle(adj: &[Vec<usize>], component: &[usize]) -> bool {
    match component {
        [only] => adj.get(*only).is_some_and(|e| e.contains(only)),
        _ => true,
    }
}

/// 循環しないノードを、依存先が先に来る順に並べる。順位が並んだら番号の小さいほうを先にする。
/// 循環に含まれるノードと、それに依存するノードは除く。
pub(crate) fn topo_order(adj: &[Vec<usize>]) -> Vec<usize> {
    let n = adj.len();
    let cyclic: BTreeSet<usize> = sccs(adj)
        .into_iter()
        .filter(|c| is_cycle(adj, c))
        .flatten()
        .collect();
    // 依存先の数（循環に依存するものは、いつまでも準備ができない）。
    let mut waiting: Vec<usize> = (0..n)
        .map(|u| {
            adj.get(u)
                .map_or(0, |deps| deps.iter().collect::<BTreeSet<_>>().len())
        })
        .collect();
    let mut dependents: Vec<Vec<usize>> = vec![Vec::new(); n];
    for (u, deps) in adj.iter().enumerate() {
        for &v in deps.iter().collect::<BTreeSet<_>>() {
            if let Some(d) = dependents.get_mut(v) {
                d.push(u);
            }
        }
    }
    let mut ready: BTreeSet<usize> = (0..n)
        .filter(|u| waiting.get(*u) == Some(&0) && !cyclic.contains(u))
        .collect();
    let mut out = Vec::new();
    while let Some(u) = ready.pop_first() {
        out.push(u);
        for &d in dependents.get(u).map_or(&[][..], Vec::as_slice) {
            if let Some(w) = waiting.get_mut(d) {
                *w = w.saturating_sub(1);
                if *w == 0 && !cyclic.contains(&d) {
                    ready.insert(d);
                }
            }
        }
    }
    out
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn finds_cycles_and_orders_the_rest() {
        // 0 → 1 → 2 → 1、3 → 3、4 → 0、5
        let adj = vec![vec![1], vec![2], vec![1], vec![3], vec![0], vec![]];
        let cycles: Vec<Vec<usize>> = sccs(&adj)
            .into_iter()
            .filter(|c| is_cycle(&adj, c))
            .collect();
        assert_eq!(cycles, [vec![1, 2], vec![3]]);
        assert_eq!(topo_order(&adj), [5]);
        // 依存先が先、順位が並んだら番号順。
        let adj = vec![vec![2], vec![], vec![1], vec![]];
        assert_eq!(topo_order(&adj), [1, 2, 0, 3]);
    }

    #[test]
    fn long_chains_do_not_overflow_the_stack() {
        let n = 200_000;
        let adj: Vec<Vec<usize>> = (0..n)
            .map(|i| if i + 1 < n { vec![i + 1] } else { vec![] })
            .collect();
        assert_eq!(sccs(&adj).len(), n);
        assert_eq!(topo_order(&adj).first(), Some(&(n - 1)));
    }
}
