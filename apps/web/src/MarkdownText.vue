<script setup>
import {computed} from 'vue'
import {renderMarkdown} from './markdown.js'
const props=defineProps({text:{type:String,default:''}})
const html=computed(()=>renderMarkdown(props.text))
</script>
<template><div class="markdown-body" v-html="html"></div></template>
