// Brush and shape gallery previews, drawn by the engine itself.

function draw(Engine, wasm, w, h, paint) {
  const e = new Engine(w, h);
  paint(e);
  e.enter();
  e.render();
  const cv = document.createElement('canvas');
  cv.width = w;
  cv.height = h;
  const data = new Uint8ClampedArray(wasm.memory.buffer, e.frame_ptr(), e.frame_len()).slice();
  // White becomes transparent so previews sit on any theme.
  for (let i = 0; i < data.length; i += 4) {
    const lum = Math.min(data[i], data[i + 1], data[i + 2]);
    data[i + 3] = 255 - lum;
    data[i] = data[i + 1] = data[i + 2] = 0;
  }
  cv.getContext('2d').putImageData(new ImageData(data, w, h), 0, 0);
  e.free();
  return cv;
}

const down = (e, x, y) => e.pointer_down(x, y, 0, false, false, false);
const move = (e, x, y) => e.pointer_move(x, y, false, false, false);
const up = (e, x, y) => e.pointer_up(x, y);

export function brushPreview(Engine, wasm, kind) {
  return draw(Engine, wasm, 72, 30, (e) => {
    e.set_brush(kind);
    e.set_size(kind === 'airbrush' ? 3 : 5);
    down(e, 6, 20);
    for (let x = 6; x <= 66; x += 2) move(e, x, 15 - Math.sin((x - 6) / 9.5) * 7);
    up(e, 66, 15);
  });
}

export function shapePreview(Engine, wasm, kind) {
  return draw(Engine, wasm, 24, 24, (e) => {
    e.set_shape(kind);
    e.set_size(1);
    if (kind === 'line') {
      down(e, 3, 20);
      move(e, 20, 3);
      up(e, 20, 3);
    } else if (kind === 'curve') {
      down(e, 3, 18);
      move(e, 20, 18);
      up(e, 20, 18);
      down(e, 7, -4);
      up(e, 7, -4);
      down(e, 15, 26);
      up(e, 15, 26);
    } else if (kind === 'polygon') {
      down(e, 4, 4);
      move(e, 18, 6);
      up(e, 18, 6);
      for (const [x, y] of [[20, 19], [9, 14], [4, 20]]) {
        down(e, x, y);
        up(e, x, y);
      }
      e.enter();
    } else {
      down(e, 2, 3);
      move(e, 21, 20);
      up(e, 21, 20);
    }
  });
}
