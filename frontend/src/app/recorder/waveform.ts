import {
  ChangeDetectionStrategy,
  Component,
  DestroyRef,
  ElementRef,
  afterNextRender,
  afterRenderEffect,
  inject,
  input,
  viewChild,
} from '@angular/core';

/** Horizontal position of the playhead (0…1 of the width). */
const PLAYHEAD = 0.6;
/** Distance between bars in CSS pixels. */
const STEP = 5;

/**
 * Recorder-style waveform: one bar per level value left of a red playhead, a dotted baseline
 * to its right. Colours come from CSS custom properties of the host.
 */
@Component({
  selector: 'app-waveform',
  template: '<canvas #canvas aria-hidden="true"></canvas>',
  styles: `
    :host {
      display: block;
      position: relative;
    }
    canvas {
      position: absolute;
      inset: 0;
      width: 100%;
      height: 100%;
    }
  `,
  changeDetection: ChangeDetectionStrategy.OnPush,
})
export class Waveform {
  readonly levels = input.required<number[]>();
  readonly active = input(false);

  private readonly canvas = viewChild.required<ElementRef<HTMLCanvasElement>>('canvas');

  constructor() {
    afterRenderEffect(() => this.draw(this.levels(), this.active()));
    // redraw when the display grows or shrinks with the window
    const destroyRef = inject(DestroyRef);
    afterNextRender(() => {
      if (typeof ResizeObserver === 'undefined') return;
      const ro = new ResizeObserver(() => this.draw(this.levels(), this.active()));
      ro.observe(this.canvas().nativeElement);
      destroyRef.onDestroy(() => ro.disconnect());
    });
  }

  private draw(levels: number[], active: boolean): void {
    const canvas = this.canvas().nativeElement;
    const dpr = globalThis.devicePixelRatio || 1;
    const { width, height } = canvas.getBoundingClientRect();
    if (width === 0 || height === 0) return;
    canvas.width = Math.round(width * dpr);
    canvas.height = Math.round(height * dpr);
    const ctx = canvas.getContext('2d');
    if (!ctx) return;
    ctx.scale(dpr, dpr);
    const css = getComputedStyle(canvas);
    const fg = css.getPropertyValue('--wave-fg').trim() || '#fff';
    const dim = css.getPropertyValue('--wave-dim').trim() || '#666';
    const head = css.getPropertyValue('--wave-head').trim() || '#e53935';

    const mid = height / 2;
    const x0 = Math.round(width * PLAYHEAD);
    const maxH = Math.min(height * 0.42, 56);

    // bars, newest at the playhead
    ctx.strokeStyle = fg;
    ctx.lineWidth = 1.5;
    ctx.lineCap = 'round';
    const count = Math.min(levels.length, Math.floor(x0 / STEP));
    for (let i = 0; i < count; i++) {
      const level = levels[levels.length - 1 - i];
      const h = Math.max(1.5, Math.min(1, Math.sqrt(level) * 2.4) * maxH);
      const x = x0 - (i + 1) * STEP;
      ctx.beginPath();
      ctx.moveTo(x, mid - h);
      ctx.lineTo(x, mid + h);
      ctx.stroke();
    }

    // dotted baseline after the playhead and where no history exists yet
    ctx.strokeStyle = dim;
    ctx.lineWidth = 1;
    const historyStart = x0 - count * STEP;
    for (let x = x0 - STEP; x > 2; x -= STEP) {
      if (x >= historyStart) continue;
      ctx.beginPath();
      ctx.moveTo(x, mid - 2);
      ctx.lineTo(x, mid + 2);
      ctx.stroke();
    }
    for (let x = x0 + STEP; x < width - 2; x += STEP) {
      ctx.beginPath();
      ctx.moveTo(x, mid - 3);
      ctx.lineTo(x, mid + 3);
      ctx.stroke();
    }

    // playhead with dot
    ctx.strokeStyle = head;
    ctx.fillStyle = head;
    ctx.lineWidth = 2;
    ctx.globalAlpha = active ? 1 : 0.55;
    ctx.beginPath();
    ctx.moveTo(x0, mid - maxH - 6);
    ctx.lineTo(x0, mid + maxH + 6);
    ctx.stroke();
    ctx.beginPath();
    ctx.arc(x0, mid - maxH - 8, 4.5, 0, Math.PI * 2);
    ctx.fill();
  }
}
