<script lang="ts">
  import { convertFileSrc } from "@tauri-apps/api/core";
  import type { BoardBackground } from "$lib/types";

  // A board's background: a built-in scene drawn with CSS/SVG (free to use,
  // made for this app), or the user's own picture or video.
  let { bg }: { bg: BoardBackground } = $props();

  let src = $derived(bg.kind === "preset" ? null : convertFileSrc(bg.value));
  let pos = $derived(`${bg.align} center`);
  // Fixed positions for the drifting lights, so they don't jump on redraw.
  const lights = Array.from({ length: 18 }, (_, i) => ({
    x: (i * 37 + 11) % 100,
    size: 6 + ((i * 7) % 18),
    delay: -((i * 2.3) % 24),
    dur: 18 + ((i * 5) % 16),
    hue: [190, 260, 320, 40][i % 4],
  }));
</script>

<div class="backdrop" aria-hidden="true">
  {#if bg.kind === "image" && src}
    <img {src} alt="" style="object-position: {pos}" />
  {:else if bg.kind === "video" && src}
    <video {src} autoplay loop muted playsinline style="object-position: {pos}"></video>
  {:else if bg.value === "aurora"}
    <div class="scene aurora"><span class="b1"></span><span class="b2"></span><span class="b3"></span></div>
  {:else if bg.value === "nebula"}
    <div class="scene nebula"><span class="stars"></span><span class="stars far"></span></div>
  {:else if bg.value === "sunset"}
    <div class="scene sunset"><span class="sun"></span></div>
  {:else if bg.value === "drift"}
    <div class="scene drift">
      {#each lights as l, i (i)}
        <span style="left: {l.x}%; width: {l.size}px; height: {l.size}px; animation-delay: {l.delay}s; animation-duration: {l.dur}s; --h: {l.hue}"></span>
      {/each}
    </div>
  {:else if bg.value === "synthwave"}
    <div class="scene synthwave"><span class="sun"></span><span class="floor"></span></div>
  {:else if bg.value === "peaks"}
    <div class="scene peaks">
      <span class="moon"></span>
      <svg viewBox="0 0 1600 600" preserveAspectRatio="xMaxYMax slice">
        <path d="M0 600 L0 430 L180 360 L340 420 L520 300 L700 390 L860 260 L1040 340 L1220 180 L1400 300 L1600 220 L1600 600Z" fill="#1d2440" />
        <path d="M0 600 L0 500 L240 430 L420 480 L640 400 L860 470 L1080 360 L1260 420 L1450 330 L1600 380 L1600 600Z" fill="#151a30" />
        <path d="M0 600 L0 560 L300 510 L560 550 L820 490 L1100 540 L1340 470 L1600 520 L1600 600Z" fill="#0e1122" />
      </svg>
    </div>
  {:else if bg.value === "ocean"}
    <div class="scene ocean">
      <svg class="wave w1" viewBox="0 0 1200 120" preserveAspectRatio="none"><path d="M0 60 Q150 20 300 60 T600 60 T900 60 T1200 60 V120 H0Z" /></svg>
      <svg class="wave w2" viewBox="0 0 1200 120" preserveAspectRatio="none"><path d="M0 70 Q150 30 300 70 T600 70 T900 70 T1200 70 V120 H0Z" /></svg>
    </div>
  {/if}
  <div class="dim" style="--dim: {bg.dim / 100}"></div>
</div>

<style>
  .backdrop {
    position: absolute;
    inset: 0;
    overflow: hidden;
    pointer-events: none;
    background: #0b0d12;
    /* Scene sizes follow the backdrop, so the picker's small previews match. */
    container-type: size;
  }
  img,
  video {
    width: 100%;
    height: 100%;
    object-fit: cover;
    display: block;
  }
  .dim {
    position: absolute;
    inset: 0;
    background:
      linear-gradient(90deg, rgb(8 9 12 / calc(var(--dim) + 0.1)) 0%, rgb(8 9 12 / var(--dim)) 55%, rgb(8 9 12 / calc(var(--dim) * 0.6)) 100%);
  }
  .scene {
    position: absolute;
    inset: 0;
  }

  /* Aurora: slow, soft colour clouds. */
  .aurora {
    background: radial-gradient(120% 90% at 70% 110%, #10233a 0%, #0b0d14 60%);
  }
  .aurora span {
    position: absolute;
    border-radius: 50%;
    filter: blur(90px);
    opacity: 0.55;
    animation: float 38s ease-in-out infinite alternate;
  }
  .aurora .b1 {
    width: 55cqw;
    height: 45cqh;
    left: 40%;
    top: 5%;
    background: #1fb5a6;
  }
  .aurora .b2 {
    width: 45cqw;
    height: 50cqh;
    left: 60%;
    top: 35%;
    background: #7b4dff;
    animation-duration: 46s;
    animation-delay: -12s;
  }
  .aurora .b3 {
    width: 40cqw;
    height: 35cqh;
    left: 10%;
    top: 55%;
    background: #2b6cff;
    opacity: 0.35;
    animation-duration: 52s;
    animation-delay: -25s;
  }
  @keyframes float {
    0% {
      transform: translate(0, 0) scale(1);
    }
    50% {
      transform: translate(-8cqw, 6cqh) scale(1.15);
    }
    100% {
      transform: translate(6cqw, -4cqh) scale(0.95);
    }
  }

  /* Nebula: deep space with a coloured cloud on the right. */
  .nebula {
    background:
      radial-gradient(40% 55% at 82% 40%, rgb(168 60 210 / 0.55), transparent 70%),
      radial-gradient(35% 45% at 70% 70%, rgb(40 110 230 / 0.45), transparent 70%),
      radial-gradient(30% 30% at 92% 15%, rgb(240 90 140 / 0.35), transparent 70%),
      #07080f;
  }
  .stars {
    position: absolute;
    inset: 0;
    background-image:
      radial-gradient(1.5px 1.5px at 20px 30px, #fff, transparent),
      radial-gradient(1px 1px at 130px 80px, #cfe, transparent),
      radial-gradient(1.5px 1.5px at 60px 160px, #fff, transparent),
      radial-gradient(1px 1px at 190px 140px, #fdf, transparent),
      radial-gradient(1px 1px at 240px 40px, #fff, transparent);
    background-size: 260px 200px;
    opacity: 0.7;
    animation: twinkle 7s ease-in-out infinite alternate;
  }
  .stars.far {
    background-size: 170px 150px;
    opacity: 0.35;
    animation-delay: -3s;
  }
  @keyframes twinkle {
    to {
      opacity: 0.25;
    }
  }

  /* Sunset: warm sky, a big low sun on the right. */
  .sunset {
    background: linear-gradient(180deg, #1a1033 0%, #4a1d4f 45%, #b24a4a 75%, #f0a35a 100%);
  }
  .sunset .sun {
    position: absolute;
    right: 8%;
    bottom: -12cqh;
    width: 42cqh;
    height: 42cqh;
    border-radius: 50%;
    background: radial-gradient(circle, #ffd98a 0%, #ff9a5a 55%, rgb(255 120 80 / 0) 72%);
    filter: blur(2px);
  }

  /* Drift: soft lights rising slowly. */
  .drift {
    background: radial-gradient(120% 100% at 80% 100%, #1a1630 0%, #0a0b12 70%);
  }
  .drift span {
    position: absolute;
    bottom: -40px;
    border-radius: 50%;
    background: hsl(var(--h) 80% 70%);
    box-shadow: 0 0 18px 6px hsl(var(--h) 80% 60% / 0.6);
    opacity: 0;
    animation: rise linear infinite;
  }
  @keyframes rise {
    0% {
      transform: translateY(0);
      opacity: 0;
    }
    15% {
      opacity: 0.55;
    }
    85% {
      opacity: 0.4;
    }
    100% {
      transform: translateY(-110cqh);
      opacity: 0;
    }
  }

  /* Synthwave: a striped sun on the right over a moving grid. */
  .synthwave {
    background: linear-gradient(180deg, #0d0221 0%, #261447 55%, #3b1b5a 62%, #0d0221 62.2%);
  }
  .synthwave .sun {
    position: absolute;
    right: 9%;
    top: 18%;
    width: 38cqh;
    height: 38cqh;
    border-radius: 50%;
    background: linear-gradient(180deg, #ffd319 0%, #ff901f 40%, #ff2975 75%, #f222ff 100%);
    mask: repeating-linear-gradient(180deg, #000 0 14px, transparent 14px 18px);
    mask-position: 0 55%;
    box-shadow: 0 0 10cqh rgb(255 41 117 / 0.5);
  }
  .synthwave .floor {
    position: absolute;
    left: -50%;
    right: -50%;
    bottom: 0;
    height: 38%;
    background-image:
      linear-gradient(rgb(242 34 255 / 0.55) 2px, transparent 2px),
      linear-gradient(90deg, rgb(242 34 255 / 0.55) 2px, transparent 2px);
    background-size: 70px 50px;
    transform: perspective(300px) rotateX(60deg);
    transform-origin: top;
    animation: grid 2.5s linear infinite;
    mask: linear-gradient(180deg, transparent 0%, #000 40%);
  }
  @keyframes grid {
    to {
      background-position: 0 50px;
    }
  }

  /* Peaks: night mountains rising to the right, a moon above them. */
  .peaks {
    background: linear-gradient(180deg, #0a0f24 0%, #1c2a52 60%, #2e3f6e 100%);
  }
  .peaks svg {
    position: absolute;
    inset: auto 0 0 0;
    width: 100%;
    height: 70%;
  }
  .peaks .moon {
    position: absolute;
    right: 14%;
    top: 12%;
    width: 15cqh;
    height: 15cqh;
    border-radius: 50%;
    background: radial-gradient(circle at 40% 40%, #f4f1e1, #cfcab0);
    box-shadow: 0 0 8cqh 1cqh rgb(244 241 225 / 0.25);
  }

  /* Ocean: calm teal water with slow waves. */
  .ocean {
    background: linear-gradient(180deg, #06141f 0%, #0b2f43 60%, #0f4a5c 100%);
  }
  .wave {
    position: absolute;
    left: 0;
    bottom: 0;
    width: 200%;
    height: 22%;
    animation: wave 22s linear infinite;
  }
  .wave path {
    fill: rgb(30 140 160 / 0.35);
  }
  .wave.w2 {
    height: 16%;
    animation-duration: 15s;
    animation-direction: reverse;
  }
  .wave.w2 path {
    fill: rgb(20 90 120 / 0.5);
  }
  @keyframes wave {
    to {
      transform: translateX(-50%);
    }
  }

  @media (prefers-reduced-motion: reduce) {
    .scene,
    .scene * {
      animation: none !important;
    }
  }
</style>
