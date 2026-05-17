export const donutPalette = [
  "rgb(224 122 95)",
  "rgb(94 132 153)",
  "rgb(214 174 105)",
  "rgb(122 168 116)",
  "rgb(166 124 168)",
  "rgb(196 132 102)",
  "rgb(140 154 184)",
  "rgb(176 168 132)",
];

export const donutColor = (i: number) => donutPalette[i % donutPalette.length];
