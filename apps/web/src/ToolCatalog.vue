<script setup>
import CapabilityLibrary from './CapabilityLibrary.vue'
defineProps({request:Function,project:String,group:String,scope:String})
defineEmits(['close','switch'])
</script>
<template><CapabilityLibrary :request="request" :project="project" :group="group" :scope="scope" kind="tool" @close="$emit('close')" @switch="$emit('switch',$event)"/></template>
