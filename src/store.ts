import { defineStore } from 'pinia';
import { ref } from 'vue';
import { invoke, isTauri } from '@tauri-apps/api/core';
import { listen } from '@tauri-apps/api/event';
import { ElMessage } from 'element-plus';
import type { Settings, Snapshot, ImportSummary } from './types';
export const defaults:Settings={stats_dir:'',auto_watch:false,session_gap_minutes:20,daily_minutes:60,focus:'',target_rank:'',prioritize_weak:true,maintain_strength:true,ai_base_url:'https://api.openai.com/v1',ai_model:'',ai_consent:false};
export const fmt=(v:number|null|undefined,digits=1)=>v==null?'—':v.toLocaleString('zh-CN',{maximumFractionDigits:digits});
export const pct=(v:number|null|undefined)=>v==null?'数据不足':`${v>0?'+':''}${v.toFixed(1)}%`;
export const time=(v:number|null|undefined)=>v==null?'—':new Date(v*1000).toLocaleString('zh-CN',{hour12:false});
export const labels:Record<string,string>={insufficient:'数据不足',improved:'稳定改善',declined:'近期下降',pb_only:'仅 PB 提升',uncertain:'继续观察'};
export const timestampLabels:Record<string,string>={explicit_completion:'明确完成时间',explicit_challenge_start:'明确开始时间',explicit_timestamp:'CSV 时间戳',filename_challenge_start:'文件名挑战时间',challenge_start_time_with_filename_date:'开始时刻 + 文件名日期'};
export const useAppStore=defineStore('app',()=>{
  const data=ref<Snapshot|null>(null),page=ref('dashboard'),loading=ref(false),syncing=ref(false),error=ref(''),selected=ref('');
  const preview=!isTauri();
  async function call<T>(cmd:string,args?:Record<string,unknown>):Promise<T>{if(preview)throw new Error('当前为浏览器界面预览。请运行 npm run desktop 使用本地数据功能。');return invoke<T>(cmd,args);}
  async function refresh(){try{error.value='';data.value=await call<Snapshot>('snapshot');}catch(e){error.value=String(e);}}
  async function init(){loading.value=true;if(!preview){await refresh();await listen('stats-synced',()=>refresh());}loading.value=false;}
  async function scan(force=false){syncing.value=true;try{const s=await call<ImportSummary>('scan_stats',{force});await refresh();ElMessage({type:s.failed?'warning':'success',message:`新增 ${s.imported} · 更新 ${s.updated} · 跳过 ${s.skipped} · 失败 ${s.failed} · 待写完 ${s.deferred}`});}catch(e){error.value=String(e);ElMessage.error(String(e));}finally{syncing.value=false;}}
  async function save(settings:Settings){await call('save_settings',{settings});await refresh();}
  function showScenario(name:string){selected.value=name;page.value='scenarios';}
  return {data,page,loading,syncing,error,selected,preview,call,refresh,init,scan,save,showScenario};
});
