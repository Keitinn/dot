import { SPRITES } from './sprites.js';
import { eyeGrid, shouldBlink } from './companion-logic.js';

const BLINK_MS = 160;

export class PetRenderer {
    constructor(canvasId) {
        this.canvas = document.getElementById(canvasId);
        this.ctx = this.canvas.getContext('2d');
        this.ctx.imageSmoothingEnabled = false;

        this.currentAnimation = 'idle';
        this.currentFrame = 0;
        this.frameTimer = 0;
        this.frameDelay = 200; // ms between frames
        this.lastFrameTime = 0;
        this.animTime = 0; // Continuous animation time

        this.running = false;

        // Companion face on the adult (see setFace)
        this.face = null;
        this.blinkUntil = 0;
        this.stillAdult = false;

        // Load sprite images (placeholder for now)
        this.sprites = {};
        this.loadSprites();
    }

    loadSprites() {
        // For now, we'll draw simple geometric shapes
        // Later we'll load actual pixel art sprite sheets
        this.spritesLoaded = true;
    }

    start(renderCallback) {
        this.running = true;
        this.renderCallback = renderCallback;
        this.lastFrameTime = performance.now();
        this.loop();
    }

    stop() {
        this.running = false;
    }

    loop(currentTime = performance.now()) {
        if (!this.running) return;

        const deltaTime = currentTime - this.lastFrameTime;
        this.lastFrameTime = currentTime;

        // Update animation time (continuous)
        this.animTime += deltaTime / 1000; // Convert to seconds

        // Update frame timing
        this.frameTimer += deltaTime;
        if (this.frameTimer >= this.frameDelay) {
            this.frameTimer = 0;
            this.currentFrame = (this.currentFrame + 1) % 4;
        }

        // Call render callback
        if (this.renderCallback) {
            this.renderCallback(deltaTime);
        }

        requestAnimationFrame((time) => this.loop(time));
    }

    render(petState) {
        // Clear canvas with off-white background (Tamagotchi screen color)
        this.ctx.fillStyle = '#c8d4c0';
        this.ctx.fillRect(0, 0, this.canvas.width, this.canvas.height);

        // Draw pixel grid effect (optional, subtle)
        this.drawPixelGrid();

        // Draw simple placeholder pet (will be replaced with actual sprites)
        this.drawPlaceholderPet(petState);
    }

    drawPixelGrid() {
        // Very subtle pixel grid for retro effect
        this.ctx.strokeStyle = 'rgba(0, 0, 0, 0.03)';
        this.ctx.lineWidth = 1;
        const gridSize = 8;

        for (let x = 0; x < this.canvas.width; x += gridSize) {
            this.ctx.beginPath();
            this.ctx.moveTo(x, 0);
            this.ctx.lineTo(x, this.canvas.height);
            this.ctx.stroke();
        }

        for (let y = 0; y < this.canvas.height; y += gridSize) {
            this.ctx.beginPath();
            this.ctx.moveTo(0, y);
            this.ctx.lineTo(this.canvas.width, y);
            this.ctx.stroke();
        }
    }

    drawPlaceholderPet(petState) {
        const x = 64;
        const y = 64;

        // Draw based on stage
        switch (petState.stage) {
            case 'Egg':
                this.drawEgg(x, y);
                break;
            case 'Baby':
                this.drawBaby(x, y);
                break;
            case 'Child':
            case 'Teen':  // Support old Teen stage
                this.drawChild(x, y);
                break;
            case 'Adult':
                this.drawAdult(x, y);
                break;
            default:
                this.drawEgg(x, y);
        }
    }

    drawEgg(x, y) {
        const pixelSize = 4;

        // Egg wobbles gently like it's about to hatch
        const wobble = Math.sin(this.animTime * 1.2) * 1.5;
        const tilt = Math.sin(this.animTime * 0.8) * 0.05;

        this.ctx.save();
        this.ctx.translate(x, y);
        this.ctx.rotate(tilt);

        this.ctx.fillStyle = '#000';

        const eggPixels = [
            [0,2],[0,3],[0,4],[0,5],
            [1,1],[1,2],[1,3],[1,4],[1,5],[1,6],
            [2,0],[2,1],[2,2],[2,3],[2,4],[2,5],[2,6],[2,7],
            [3,0],[3,1],[3,2],[3,3],[3,4],[3,5],[3,6],[3,7],
            [4,1],[4,2],[4,3],[4,4],[4,5],[4,6],
            [5,2],[5,3],[5,4],[5,5]
        ];

        eggPixels.forEach(([px, py]) => {
            this.ctx.fillRect(
                -12 + px * pixelSize + wobble,
                -16 + py * pixelSize,
                pixelSize,
                pixelSize
            );
        });

        this.ctx.restore();
    }

    drawBaby(x, y) {
        const pixelSize = 4;

        // Baby does gentle little hops
        const hopCycle = (this.animTime * 1.2) % 1;
        const hop = hopCycle < 0.3 ? Math.sin(hopCycle / 0.3 * Math.PI) * 3 : 0;
        const squash = hopCycle < 0.1 ? 0.95 : (hopCycle > 0.25 && hopCycle < 0.35 ? 1.05 : 1);

        this.ctx.save();
        this.ctx.translate(x, y - hop);
        this.ctx.scale(1, squash);

        this.ctx.fillStyle = '#000';

        const babyPixels = [
            [1,1],[1,2],[1,3],[1,4],[1,5],[1,6],
            [2,0],[2,1],[2,2],[2,3],[2,4],[2,5],[2,6],[2,7],
            [3,0],[3,1],[3,2],[3,3],[3,4],[3,5],[3,6],[3,7],
            [4,1],[4,2],[4,3],[4,4],[4,5],[4,6]
        ];

        babyPixels.forEach(([px, py]) => {
            this.ctx.fillRect(
                -10 + px * pixelSize,
                -14 + py * pixelSize,
                pixelSize,
                pixelSize
            );
        });

        // Eyes
        this.ctx.fillStyle = '#c8d4c0';
        this.ctx.fillRect(-6, -6, pixelSize, pixelSize);
        this.ctx.fillRect(2, -6, pixelSize, pixelSize);

        this.ctx.fillStyle = '#000';
        this.ctx.fillRect(-4, -4, pixelSize/2, pixelSize/2);
        this.ctx.fillRect(4, -4, pixelSize/2, pixelSize/2);

        this.ctx.restore();
    }

    drawChild(x, y) {
        const pixelSize = 4;

        // Child sways gently side to side
        const wiggle = Math.sin(this.animTime * 1.5) * 2;
        const tilt = Math.sin(this.animTime * 1.5) * 0.04;

        this.ctx.save();
        this.ctx.translate(x + wiggle, y);
        this.ctx.rotate(tilt);

        this.ctx.fillStyle = '#000';

        const childPixels = [
            [0,2],[0,3],
            [6,2],[6,3],
            [1,1],[1,2],[1,3],[1,4],[1,5],[1,6],[1,7],
            [2,0],[2,1],[2,2],[2,3],[2,4],[2,5],[2,6],[2,7],[2,8],
            [3,0],[3,1],[3,2],[3,3],[3,4],[3,5],[3,6],[3,7],[3,8],
            [4,0],[4,1],[4,2],[4,3],[4,4],[4,5],[4,6],[4,7],[4,8],
            [5,1],[5,2],[5,3],[5,4],[5,5],[5,6],[5,7]
        ];

        childPixels.forEach(([px, py]) => {
            this.ctx.fillRect(
                -14 + px * pixelSize,
                -18 + py * pixelSize,
                pixelSize,
                pixelSize
            );
        });

        // Eyes
        this.ctx.fillStyle = '#c8d4c0';
        this.ctx.fillRect(-8, -8, pixelSize, pixelSize);
        this.ctx.fillRect(4, -8, pixelSize, pixelSize);

        this.ctx.fillStyle = '#000';
        this.ctx.fillRect(-6, -6, pixelSize/2, pixelSize/2);
        this.ctx.fillRect(6, -6, pixelSize/2, pixelSize/2);

        this.ctx.restore();
    }

    drawAdult(x, y) {
        const pixelSize = 4;

        // Adult has gentle breathing/swaying motion - calm and mature.
        // Held still for the companion when motion is reduced.
        const breathe = this.stillAdult ? 0 : Math.sin(this.animTime * 1.0) * 1.5;
        const sway = this.stillAdult ? 0 : Math.sin(this.animTime * 0.5) * 1;

        this.ctx.save();
        this.ctx.translate(x + sway, y + breathe);

        this.ctx.fillStyle = '#000';

        const adultPixels = [
            [0,3],[0,4],[0,5],
            [1,2],[1,3],[1,4],
            [7,2],[7,3],[7,4],
            [8,3],[8,4],[8,5],
            [1,5],[1,6],[1,7],[1,8],[1,9],
            [2,1],[2,2],[2,3],[2,4],[2,5],[2,6],[2,7],[2,8],[2,9],[2,10],
            [3,0],[3,1],[3,2],[3,3],[3,4],[3,5],[3,6],[3,7],[3,8],[3,9],[3,10],
            [4,0],[4,1],[4,2],[4,3],[4,4],[4,5],[4,6],[4,7],[4,8],[4,9],[4,10],
            [5,0],[5,1],[5,2],[5,3],[5,4],[5,5],[5,6],[5,7],[5,8],[5,9],[5,10],
            [6,1],[6,2],[6,3],[6,4],[6,5],[6,6],[6,7],[6,8],[6,9],[6,10],
            [7,5],[7,6],[7,7],[7,8],[7,9]
        ];

        adultPixels.forEach(([px, py]) => {
            this.ctx.fillRect(
                -18 + px * pixelSize,
                -22 + py * pixelSize,
                pixelSize,
                pixelSize
            );
        });

        if (this.face) {
            const shape = performance.now() < this.blinkUntil ? 'blink' : this.face;
            this.drawAdultEye(-10, -10, shape);
            this.drawAdultEye(6, -10, shape);
            this.ctx.restore();
            return;
        }

        // Eyes
        this.ctx.fillStyle = '#c8d4c0';
        this.ctx.fillRect(-10, -10, pixelSize * 1.5, pixelSize * 1.5);
        this.ctx.fillRect(6, -10, pixelSize * 1.5, pixelSize * 1.5);

        this.ctx.fillStyle = '#000';
        this.ctx.fillRect(-8, -8, pixelSize, pixelSize);
        this.ctx.fillRect(8, -8, pixelSize, pixelSize);

        this.ctx.restore();
    }

    // Only the 6x6 eye is redrawn; the body around it stays as it is.
    drawAdultEye(x, y, shape) {
        const cell = 2;
        this.ctx.fillStyle = '#000';
        this.ctx.fillRect(x, y, cell * 3, cell * 3);
        this.ctx.fillStyle = '#c8d4c0';
        eyeGrid(shape).forEach((row, cy) => {
            row.forEach((open, cx) => {
                if (open) this.ctx.fillRect(x + cx * cell, y + cy * cell, cell, cell);
            });
        });
    }

    /**
     * The companion mood to show on the adult, or null for the usual eyes.
     * A new face blinks once unless motion is reduced. A still face (the
     * farewell's closed eyes) neither blinks nor breathes.
     */
    setFace(mood, { reducedMotion = false, still = false } = {}) {
        if (still) {
            this.blinkUntil = 0;
        } else if (shouldBlink(this.face, mood, reducedMotion)) {
            this.blinkUntil = performance.now() + BLINK_MS;
        }
        this.face = mood;
        this.stillAdult = mood !== null && (reducedMotion || still);
    }
}
