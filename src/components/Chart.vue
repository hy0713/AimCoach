<script setup lang="ts">
import { ref,onMounted,onBeforeUnmount,watch } from 'vue';
import * as echarts from 'echarts/core';
import { LineChart,BarChart,RadarChart } from 'echarts/charts';
import { GridComponent,TooltipComponent,LegendComponent,RadarComponent } from 'echarts/components';
import { CanvasRenderer } from 'echarts/renderers';
import type { EChartsCoreOption } from 'echarts/core';
echarts.use([LineChart,BarChart,RadarChart,GridComponent,TooltipComponent,LegendComponent,RadarComponent,CanvasRenderer]);
const props=defineProps<{option:EChartsCoreOption;height?:number}>();const host=ref<HTMLDivElement>();let chart:echarts.ECharts|undefined;let observer:ResizeObserver;
onMounted(()=>{chart=echarts.init(host.value!,'dark');chart.setOption({...props.option,backgroundColor:'transparent'});observer=new ResizeObserver(()=>chart?.resize());observer.observe(host.value!);});
watch(()=>props.option,v=>chart?.setOption({...v,backgroundColor:'transparent'},true),{deep:true});
onBeforeUnmount(()=>{observer?.disconnect();chart?.dispose();});
</script>
<template><div ref="host" :style="{height:`${height??320}px`,width:'100%'}" /></template>
