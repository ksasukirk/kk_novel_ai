<!--
  统一胶囊开关
  代码路径: kk_novel_ai/src/components/CapsuleSwitch.vue
-->
<script setup>
defineProps({
  modelValue: { type: Boolean, default: false },
  label: { type: String, default: "" },
  disabled: { type: Boolean, default: false },
});

const emit = defineEmits(["update:modelValue"]);

function onChange(e) {
  emit("update:modelValue", !!e.target.checked);
}
</script>

<template>
  <label class="capsule-switch" :class="{ 'is-disabled': disabled }">
    <input
      type="checkbox"
      class="capsule-switch-input"
      :checked="modelValue"
      :disabled="disabled"
      @change="onChange"
    />
    <span class="capsule-switch-track" aria-hidden="true"></span>
    <span v-if="label" class="capsule-switch-text">{{ label }}</span>
    <slot />
  </label>
</template>
