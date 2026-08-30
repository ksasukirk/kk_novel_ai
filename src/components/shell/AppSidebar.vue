<!--
  侧栏：展开 / 图标栏 / 隐藏 + 主题切换
  代码路径: kk_novel_ai/src/components/shell/AppSidebar.vue
-->
<script setup>
defineProps({
  /** expanded | compact | closed */
  mode: { type: String, default: "expanded" },
  tabs: { type: Array, required: true },
  activeId: { type: String, required: true },
  theme: { type: String, default: "light" },
  themeLabel: { type: String, default: "切换主题" },
  overlay: { type: Boolean, default: false },
});

defineEmits(["select", "toggle-theme"]);
</script>

<template>
  <aside
    class="app-sidebar"
    :class="{
      'is-expanded': mode === 'expanded',
      'is-compact': mode === 'compact',
      'is-closed': mode === 'closed',
      'is-overlay': overlay,
    }"
  >
    <div class="sidebar-inner" :class="{ 'is-compact': mode === 'compact' }">
      <div class="sidebar-brand">
        <div class="brand-mark" aria-hidden="true">K</div>
        <div v-if="mode !== 'compact'" class="brand-text">
          <span class="brand-kk">Kk</span>
          <span class="brand-grad-text">Novel</span>
        </div>
      </div>

      <nav class="sidebar-nav">
        <button
          v-for="tab in tabs"
          :key="tab.id"
          type="button"
          class="nav-pill"
          :class="activeId === tab.id ? 'nav-pill-active' : ''"
          :title="tab.label"
          @click="$emit('select', tab)"
        >
          <span class="nav-glyph" :data-icon="tab.id" aria-hidden="true"></span>
          <span v-if="mode !== 'compact'" class="nav-label">{{ tab.label }}</span>
          <span v-if="mode !== 'compact'" class="nav-indicator" aria-hidden="true"></span>
        </button>
      </nav>

      <div class="sidebar-foot">
        <button
          type="button"
          class="app-btn app-btn-light theme-btn"
          :class="{ 'theme-btn-compact': mode === 'compact' }"
          @click="$emit('toggle-theme')"
          :title="themeLabel"
        >
          <span
            class="theme-glyph"
            :class="theme === 'dark' ? 'is-sun' : 'is-moon'"
            aria-hidden="true"
          ></span>
          <span v-if="mode !== 'compact'" class="theme-btn-label">{{ themeLabel }}</span>
        </button>
        <p v-if="mode !== 'compact'" class="cli-hint">kk_novel_ai &lt;cmd&gt;</p>
      </div>
    </div>
  </aside>
</template>

<style scoped>
.app-sidebar {
  flex-shrink: 0;
  width: 0;
  overflow: hidden;
  transition: width 0.28s cubic-bezier(0.34, 1.2, 0.64, 1);
  background: var(--sidebar-bg);
  z-index: 30;
}

.app-sidebar.is-expanded {
  width: var(--sidebar-w);
}

.app-sidebar.is-compact {
  width: var(--sidebar-w-compact);
}

.app-sidebar.is-closed {
  width: 0;
}

.app-sidebar.is-overlay {
  position: absolute;
  left: 0;
  top: 0;
  bottom: 0;
  z-index: 30;
  box-shadow: var(--shadow);
  background: var(--panel);
  transition: transform 0.28s cubic-bezier(0.34, 1.2, 0.64, 1), width 0.28s cubic-bezier(0.34, 1.2, 0.64, 1);
}

.app-sidebar.is-overlay.is-closed {
  width: var(--sidebar-w);
  transform: translateX(-105%);
  pointer-events: none;
}

.app-sidebar.is-overlay.is-expanded,
.app-sidebar.is-overlay.is-compact {
  transform: translateX(0);
}

.app-sidebar.is-overlay.is-expanded {
  width: var(--sidebar-w);
}

.app-sidebar.is-overlay.is-compact {
  width: var(--sidebar-w-compact);
}

.sidebar-inner {
  width: var(--sidebar-w);
  height: 100%;
  display: flex;
  flex-direction: column;
  padding: 10px 12px 12px;
}

.sidebar-inner.is-compact {
  width: var(--sidebar-w-compact);
  padding: 10px 6px 12px;
  align-items: center;
}

.sidebar-brand {
  display: flex;
  align-items: center;
  justify-content: center;
  gap: 8px;
  padding: 8px 4px 16px;
}

.brand-mark {
  width: 36px;
  height: 36px;
  border-radius: 12px;
  background: linear-gradient(135deg, #f472b6, #c084fc);
  color: #fff;
  display: inline-flex;
  align-items: center;
  justify-content: center;
  font-size: 15px;
  font-weight: 700;
  box-shadow: var(--shadow-sm);
  flex-shrink: 0;
}

.brand-text {
  display: flex;
  align-items: baseline;
  gap: 4px;
  font-size: 1.35rem;
  line-height: 1;
}

.brand-kk {
  font-weight: 700;
  color: var(--text);
}

.sidebar-nav {
  display: flex;
  flex-direction: column;
  gap: 6px;
  flex: 1;
  min-height: 0;
  overflow-y: auto;
  padding: 0 4px;
  width: 100%;
}

.sidebar-inner.is-compact .sidebar-nav {
  padding: 0;
  align-items: center;
}

.nav-glyph {
  width: 16px;
  height: 16px;
  flex-shrink: 0;
  border-radius: 4px;
  position: relative;
  background: currentColor;
  opacity: 0.85;
  mask-size: contain;
  mask-repeat: no-repeat;
  mask-position: center;
  -webkit-mask-size: contain;
  -webkit-mask-repeat: no-repeat;
  -webkit-mask-position: center;
}

.nav-glyph[data-icon="project"] {
  -webkit-mask-image: url("data:image/svg+xml,%3Csvg xmlns='http://www.w3.org/2000/svg' viewBox='0 0 24 24' fill='none' stroke='black' stroke-width='2'%3E%3Cpath d='M4 19.5A2.5 2.5 0 0 1 6.5 17H20'/%3E%3Cpath d='M6.5 2H20v20H6.5A2.5 2.5 0 0 1 4 19.5v-15A2.5 2.5 0 0 1 6.5 2z'/%3E%3C/svg%3E");
  mask-image: url("data:image/svg+xml,%3Csvg xmlns='http://www.w3.org/2000/svg' viewBox='0 0 24 24' fill='none' stroke='black' stroke-width='2'%3E%3Cpath d='M4 19.5A2.5 2.5 0 0 1 6.5 17H20'/%3E%3Cpath d='M6.5 2H20v20H6.5A2.5 2.5 0 0 1 4 19.5v-15A2.5 2.5 0 0 1 6.5 2z'/%3E%3C/svg%3E");
}

.nav-glyph[data-icon="story"] {
  -webkit-mask-image: url("data:image/svg+xml,%3Csvg xmlns='http://www.w3.org/2000/svg' viewBox='0 0 24 24' fill='none' stroke='black' stroke-width='2'%3E%3Ccircle cx='12' cy='12' r='3'/%3E%3Ccircle cx='5' cy='7' r='2'/%3E%3Ccircle cx='19' cy='7' r='2'/%3E%3Ccircle cx='5' cy='17' r='2'/%3E%3Ccircle cx='19' cy='17' r='2'/%3E%3Cpath d='M7 8l3 2M14 10l3-2M7 16l3-2M14 14l3 2'/%3E%3C/svg%3E");
  mask-image: url("data:image/svg+xml,%3Csvg xmlns='http://www.w3.org/2000/svg' viewBox='0 0 24 24' fill='none' stroke='black' stroke-width='2'%3E%3Ccircle cx='12' cy='12' r='3'/%3E%3Ccircle cx='5' cy='7' r='2'/%3E%3Ccircle cx='19' cy='7' r='2'/%3E%3Ccircle cx='5' cy='17' r='2'/%3E%3Ccircle cx='19' cy='17' r='2'/%3E%3Cpath d='M7 8l3 2M14 10l3-2M7 16l3-2M14 14l3 2'/%3E%3C/svg%3E");
}

.nav-glyph[data-icon="outline"] {
  -webkit-mask-image: url("data:image/svg+xml,%3Csvg xmlns='http://www.w3.org/2000/svg' viewBox='0 0 24 24' fill='none' stroke='black' stroke-width='2'%3E%3Cline x1='8' y1='6' x2='21' y2='6'/%3E%3Cline x1='8' y1='12' x2='21' y2='12'/%3E%3Cline x1='8' y1='18' x2='21' y2='18'/%3E%3Cline x1='3' y1='6' x2='3.01' y2='6'/%3E%3Cline x1='3' y1='12' x2='3.01' y2='12'/%3E%3Cline x1='3' y1='18' x2='3.01' y2='18'/%3E%3C/svg%3E");
  mask-image: url("data:image/svg+xml,%3Csvg xmlns='http://www.w3.org/2000/svg' viewBox='0 0 24 24' fill='none' stroke='black' stroke-width='2'%3E%3Cline x1='8' y1='6' x2='21' y2='6'/%3E%3Cline x1='8' y1='12' x2='21' y2='12'/%3E%3Cline x1='8' y1='18' x2='21' y2='18'/%3E%3Cline x1='3' y1='6' x2='3.01' y2='6'/%3E%3Cline x1='3' y1='12' x2='3.01' y2='12'/%3E%3Cline x1='3' y1='18' x2='3.01' y2='18'/%3E%3C/svg%3E");
}

.nav-glyph[data-icon="editor"] {
  -webkit-mask-image: url("data:image/svg+xml,%3Csvg xmlns='http://www.w3.org/2000/svg' viewBox='0 0 24 24' fill='none' stroke='black' stroke-width='2'%3E%3Cpath d='M12 20h9'/%3E%3Cpath d='M16.5 3.5a2.12 2.12 0 0 1 3 3L7 19l-4 1 1-4L16.5 3.5z'/%3E%3C/svg%3E");
  mask-image: url("data:image/svg+xml,%3Csvg xmlns='http://www.w3.org/2000/svg' viewBox='0 0 24 24' fill='none' stroke='black' stroke-width='2'%3E%3Cpath d='M12 20h9'/%3E%3Cpath d='M16.5 3.5a2.12 2.12 0 0 1 3 3L7 19l-4 1 1-4L16.5 3.5z'/%3E%3C/svg%3E");
}

.nav-glyph[data-icon="lore"] {
  -webkit-mask-image: url("data:image/svg+xml,%3Csvg xmlns='http://www.w3.org/2000/svg' viewBox='0 0 24 24' fill='none' stroke='black' stroke-width='2'%3E%3Ccircle cx='12' cy='8' r='4'/%3E%3Cpath d='M4 20c0-4 4-6 8-6s8 2 8 6'/%3E%3C/svg%3E");
  mask-image: url("data:image/svg+xml,%3Csvg xmlns='http://www.w3.org/2000/svg' viewBox='0 0 24 24' fill='none' stroke='black' stroke-width='2'%3E%3Ccircle cx='12' cy='8' r='4'/%3E%3Cpath d='M4 20c0-4 4-6 8-6s8 2 8 6'/%3E%3C/svg%3E");
}

.nav-glyph[data-icon="characters"] {
  -webkit-mask-image: url("data:image/svg+xml,%3Csvg xmlns='http://www.w3.org/2000/svg' viewBox='0 0 24 24' fill='none' stroke='black' stroke-width='2'%3E%3Cpath d='M17 21v-2a4 4 0 0 0-4-4H5a4 4 0 0 0-4 4v2'/%3E%3Ccircle cx='9' cy='7' r='4'/%3E%3Cpath d='M23 21v-2a4 4 0 0 0-3-3.87'/%3E%3Cpath d='M16 3.13a4 4 0 0 1 0 7.75'/%3E%3C/svg%3E");
  mask-image: url("data:image/svg+xml,%3Csvg xmlns='http://www.w3.org/2000/svg' viewBox='0 0 24 24' fill='none' stroke='black' stroke-width='2'%3E%3Cpath d='M17 21v-2a4 4 0 0 0-4-4H5a4 4 0 0 0-4 4v2'/%3E%3Ccircle cx='9' cy='7' r='4'/%3E%3Cpath d='M23 21v-2a4 4 0 0 0-3-3.87'/%3E%3Cpath d='M16 3.13a4 4 0 0 1 0 7.75'/%3E%3C/svg%3E");
}

.nav-glyph[data-icon="knowledge"] {
  -webkit-mask-image: url("data:image/svg+xml,%3Csvg xmlns='http://www.w3.org/2000/svg' viewBox='0 0 24 24' fill='none' stroke='black' stroke-width='2'%3E%3Cellipse cx='12' cy='5' rx='8' ry='3'/%3E%3Cpath d='M4 5v6c0 1.7 3.6 3 8 3s8-1.3 8-3V5'/%3E%3Cpath d='M4 11v6c0 1.7 3.6 3 8 3s8-1.3 8-3v-6'/%3E%3C/svg%3E");
  mask-image: url("data:image/svg+xml,%3Csvg xmlns='http://www.w3.org/2000/svg' viewBox='0 0 24 24' fill='none' stroke='black' stroke-width='2'%3E%3Cellipse cx='12' cy='5' rx='8' ry='3'/%3E%3Cpath d='M4 5v6c0 1.7 3.6 3 8 3s8-1.3 8-3V5'/%3E%3Cpath d='M4 11v6c0 1.7 3.6 3 8 3s8-1.3 8-3v-6'/%3E%3C/svg%3E");
}

.nav-glyph[data-icon="log"] {
  -webkit-mask-image: url("data:image/svg+xml,%3Csvg xmlns='http://www.w3.org/2000/svg' viewBox='0 0 24 24' fill='none' stroke='black' stroke-width='2'%3E%3Cpath d='M14 2H6a2 2 0 0 0-2 2v16a2 2 0 0 0 2 2h12a2 2 0 0 0 2-2V8z'/%3E%3Cpolyline points='14 2 14 8 20 8'/%3E%3Cline x1='16' y1='13' x2='8' y2='13'/%3E%3Cline x1='16' y1='17' x2='8' y2='17'/%3E%3C/svg%3E");
  mask-image: url("data:image/svg+xml,%3Csvg xmlns='http://www.w3.org/2000/svg' viewBox='0 0 24 24' fill='none' stroke='black' stroke-width='2'%3E%3Cpath d='M14 2H6a2 2 0 0 0-2 2v16a2 2 0 0 0 2 2h12a2 2 0 0 0 2-2V8z'/%3E%3Cpolyline points='14 2 14 8 20 8'/%3E%3Cline x1='16' y1='13' x2='8' y2='13'/%3E%3Cline x1='16' y1='17' x2='8' y2='17'/%3E%3C/svg%3E");
}

.nav-glyph[data-icon="settings"] {
  -webkit-mask-image: url("data:image/svg+xml,%3Csvg xmlns='http://www.w3.org/2000/svg' viewBox='0 0 24 24' fill='none' stroke='black' stroke-width='2'%3E%3Ccircle cx='12' cy='12' r='3'/%3E%3Cpath d='M12 1v2M12 21v2M4.2 4.2l1.4 1.4M18.4 18.4l1.4 1.4M1 12h2M21 12h2M4.2 19.8l1.4-1.4M18.4 5.6l1.4-1.4'/%3E%3C/svg%3E");
  mask-image: url("data:image/svg+xml,%3Csvg xmlns='http://www.w3.org/2000/svg' viewBox='0 0 24 24' fill='none' stroke='black' stroke-width='2'%3E%3Ccircle cx='12' cy='12' r='3'/%3E%3Cpath d='M12 1v2M12 21v2M4.2 4.2l1.4 1.4M18.4 18.4l1.4 1.4M1 12h2M21 12h2M4.2 19.8l1.4-1.4M18.4 5.6l1.4-1.4'/%3E%3C/svg%3E");
}

.nav-label {
  flex: 1;
  text-align: left;
}

.nav-indicator {
  width: 10px;
  height: 3px;
  border-radius: 999px;
  background: rgba(154, 146, 160, 0.45);
  flex-shrink: 0;
}

.nav-pill-active .nav-indicator {
  width: 3px;
  height: 14px;
  background: #fff;
  box-shadow: 0 0 0 1px rgba(255, 255, 255, 0.35);
}

.sidebar-inner.is-compact .nav-pill {
  width: 42px;
  min-width: 42px;
  justify-content: center;
  padding-left: 0;
  padding-right: 0;
}

.sidebar-inner.is-compact .nav-pill-active .nav-glyph {
  opacity: 1;
}

.sidebar-foot {
  margin-top: auto;
  padding-top: 12px;
  display: flex;
  flex-direction: column;
  gap: 6px;
  width: 100%;
}

.theme-btn {
  width: 100%;
  justify-content: center;
  display: inline-flex;
  align-items: center;
  gap: 6px;
  min-height: 40px;
}

.theme-btn-compact {
  width: 42px;
  min-width: 42px;
  padding: 0;
}

.theme-glyph {
  width: 14px;
  height: 14px;
  border-radius: 999px;
  border: 1.5px solid currentColor;
  position: relative;
  flex-shrink: 0;
}

.theme-glyph.is-moon::after {
  content: "";
  position: absolute;
  right: -2px;
  top: -2px;
  width: 10px;
  height: 10px;
  border-radius: 999px;
  background: var(--bg);
}

.theme-glyph.is-sun {
  background: radial-gradient(circle at center, currentColor 35%, transparent 36%);
}

.cli-hint {
  margin: 0;
  text-align: center;
  font-size: 11px;
  color: var(--muted);
  opacity: 0.75;
}
</style>
