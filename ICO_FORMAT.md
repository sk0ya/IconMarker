# ICO フォーマット知見まとめ

## ICOファイルの構造

```
ICONDIR (6 bytes)
  idReserved : WORD = 0
  idType     : WORD = 1 (ICO) / 2 (CUR)
  idCount    : WORD = 画像の数

ICONDIRENTRY × idCount (各16 bytes)
  bWidth       : BYTE  (0 = 256を意味する)
  bHeight      : BYTE  (0 = 256を意味する)
  bColorCount  : BYTE  (0 = 32-bitの場合)
  bReserved    : BYTE  = 0
  wPlanes      : WORD
  wBitCount    : WORD
  dwBytesInRes : DWORD (画像データのバイト数)
  dwImageOffset: DWORD (ファイル先頭からのオフセット)

画像データ × idCount
  各エントリはBMP DIBまたはPNGファイル本体
```

---

## 画像データの形式

### 旧来の BMP DIB（1/4/8-bit）

Windows初期からの形式。ANDマスク必須。

```
BITMAPINFOHEADER
  biHeight = 実際の高さ × 2  ← XOR+ANDマスク両方を含む
  biBitCount = 1 / 4 / 8
  ...

XOR mask: ピクセルデータ（下から上）
AND mask: 1-bit透明マスク（下から上、DWORD境界パディング）
  - 0 = 不透明（このピクセルを表示）
  - 1 = 透明
```

### Vista-style 32-bit BMP DIB ← **このプロジェクトで使用**

Windows Vista以降に普及した形式。ANDマスク不要。

```
BITMAPINFOHEADER
  biHeight    = 実際の高さ（2倍にしない）
  biBitCount  = 32
  biSizeImage = width × height × 4
  ...

ピクセルデータ: BGRA、下から上の行順
ANDマスク: なし（透明度はアルファチャンネルで管理）
```

### PNG埋め込み（256×256）

Windows Vista以降、256×256サイズ専用。

```
完全なPNGファイルをそのまま格納（ICODIREENTRYが指すデータがPNG）
ICODIREENTRYの bWidth/bHeight は 0（= 256を意味する）
```

---

## WPF / WIC での注意点

- WPF（System.Windows.Media.Imaging）はWIC（Windows Imaging Component）でICOを読む
- WICは32-bitエントリに対して **Vista-style**（biHeight = 実際の高さ、ANDマスクなし）を期待する
- 旧来の biHeight = 2×実際の高さ + ANDマスク方式で書くと、WICが「データサイズが合わない」と判断し `WINCODEC_ERR_BADIMAGE (0x88982F60)` を返す
- エラーはXAMLで `System.Windows.Markup.XamlParseException` として現れる

---

## このプロジェクトでの構成

| サイズ    | 形式                    |
|---------|-------------------------|
| 16×16   | Vista-style 32-bit BMP  |
| 32×32   | Vista-style 32-bit BMP  |
| 48×48   | Vista-style 32-bit BMP  |
| 256×256 | PNG埋め込み              |

---

## 過去のバグ履歴

**症状**: WPFでICOを読み込むと `0x88982F60` エラー

**原因**: 32-bitBMPエントリで `biHeight = 2×s` + ANDマスクを書いていた。
WICが biHeight=64（32×32の場合）を見て「64行分のピクセルデータが必要」と判断するが、
実際には32行分しかなくデータ不足でBADIMAGEになる。

**修正**: `biHeight = s`（実際の高さ）にして、ANDマスクを削除。
