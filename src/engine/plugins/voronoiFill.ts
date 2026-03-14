import { Delaunay } from "d3-delaunay";
import type { PipelineContext, PipelinePlugin } from "../types.js";
import { PrismError } from "../types.js";

/** Returns the flat RGBA buffer index for pixel (x, y). */
const idx = (x: number, y: number, width: number) => (y * width + x) * 4;

const NEIGHBOR_OFFSETS: readonly [number, number][] = [
    [-1, -1], [0, -1], [1, -1],
    [ 1,  0], [1,  1], [0,  1],
    [-1,  1], [-1,  0],
];

/**
 * Voronoi fill stage.
 *
 * Scans opaque pixels that border a transparent neighbour to build a set of
 * "seed" points. Then, for every transparent pixel, finds the nearest seed via
 * Delaunay/Voronoi and assigns that seed's colour — keeping alpha at 0 so the
 * transparency itself is untouched (only the hidden RGB data is corrected).
 */
export const voronoiFillPlugin: PipelinePlugin = {
    name: "voronoi-fill",

    async run(ctx: PipelineContext): Promise<void> {
        const { pixels, width, height } = ctx;

        if (!pixels) {
            throw new PrismError("voronoi-fill: pixel buffer is null (load stage must run first)");
        }

        // ── 1. Collect edge-adjacent opaque pixels as Voronoi seeds ──────────
        const seedPoints: [number, number][] = [];
        const seedColors: [number, number, number][] = [];

        for (let y = 1; y < height - 1; y++) {
            for (let x = 1; x < width - 1; x++) {
                const i = idx(x, y, width);
                if (pixels[i + 3] === 0) continue; // skip transparent pixels

                let isBorderPixel = false;
                for (const [dx, dy] of NEIGHBOR_OFFSETS) {
                    const ni = idx(x + dx, y + dy, width);
                    if (pixels[ni + 3] === 0) {
                        isBorderPixel = true;
                        break;
                    }
                }

                if (isBorderPixel) {
                    seedPoints.push([x, y]);
                    seedColors.push([pixels[i], pixels[i + 1], pixels[i + 2]]);
                }
            }
        }

        if (seedPoints.length === 0) {
            ctx.skipped = true;
            return;
        }

        // ── 2. Build Delaunay triangulation over seed points ──────────────────
        const delaunay = Delaunay.from(seedPoints);

        // ── 3. Fill every transparent pixel with its nearest seed colour ──────
        let replaced = 0;

        for (let y = 0; y < height; y++) {
            for (let x = 0; x < width; x++) {
                const i = idx(x, y, width);
                if (pixels[i + 3] !== 0) continue; // already opaque

                const nearest = delaunay.find(x, y);
                if (nearest == null || !seedColors[nearest]) continue;

                const [r, g, b] = seedColors[nearest];
                pixels[i]     = r;
                pixels[i + 1] = g;
                pixels[i + 2] = b;
                // alpha stays 0 — we're only fixing the hidden RGB data

                replaced++;
            }
        }

        ctx.replacedCount = replaced;
    },
};
