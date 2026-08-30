<!--
  装饰背景色块（CSS blob，对齐 NapCat page_background）
  代码路径: kk_novel_ai/src/components/shell/PageBackground.vue
-->
<template>
  <div class="page-bg" aria-hidden="true">
    <div class="blob blob-a"></div>
    <div class="blob blob-b"></div>
  </div>
</template>

<style scoped>
.page-bg {
  position: absolute;
  inset: 0;
  overflow: hidden;
  pointer-events: none;
  z-index: 0;
}

.blob {
  position: absolute;
  border-radius: 40% 60% 55% 45%;
  filter: blur(40px);
  opacity: 0.8;
}

.blob-a {
  width: 55%;
  height: 70%;
  top: -25%;
  right: -22%;
  transform: rotate(-45deg);
  background: var(--blob-a);
}

.blob-b {
  width: 50%;
  height: 65%;
  bottom: -30%;
  left: -18%;
  transform: rotate(90deg);
  background: var(--blob-b);
  filter: blur(48px) hue-rotate(40deg);
}
</style>
