<script setup lang="ts">
import {computed} from 'vue';import Chart from './Chart.vue';import type { BenchmarkView } from '../types';
const props=defineProps<{benchmark:BenchmarkView}>();const option=computed(()=>{const rows=props.benchmark.categories.filter(c=>c.mean!=null);return {tooltip:{trigger:'axis'},grid:{left:170,right:30,top:20,bottom:30},xAxis:{type:'value',min:0,max:props.benchmark.config.ranks.length,name:'等级归一化'},yAxis:{type:'category',data:rows.map(c=>c.name)},series:[{type:'bar',data:rows.map(c=>c.mean),barWidth:18,itemStyle:{color:'#91edae',borderRadius:[0,6,6,0]}}]};});
</script>
<template><Chart v-if="benchmark.categories.some(c=>c.mean!=null)" :option="option" :height="260"/><div v-else class="empty compact">尚无匹配的训练记录，能力图会在导入后显示。</div></template>
