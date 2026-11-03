# 参照ホスト

適合テストは、次のホストのもとで動かす。実装は、このスキーマを自分のホスト API で登録してテストを走らせる。
ここに書く型は、CDL から見た型の説明である（DECISIONS E1）。

## `host:contrast`

ルート要素: `Contrast`

```ts
export declare function Contrast(props: {
  children?: Element<Appearance | Animation | Keybind>[];
}): Element<Contrast>;

// 一意な子: Contrast の子の中に 1 つまで（DECISIONS E2）
export declare function Appearance(props: {
  gap: Length; radius: Length; focusColor?: Color;
}): Element<Appearance>;

export declare function Animation(props: {
  duration: Duration; enabled?: boolean;
}): Element<Animation>;

export declare function Keybind(props: {
  keys: string; action: string;
}): Element<Keybind>;

export interface WindowInfo { appId: string; workspace: number }
export interface Placement { workspace: number; floating: boolean }
export interface Policy {
  windowOpen?: (w: WindowInfo) => Placement;
}
```

一意な子: `Appearance`。
