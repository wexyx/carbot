import {createRouter,createWebHashHistory} from 'vue-router'
import App from './App.vue'
export const router=createRouter({
 history:createWebHashHistory(),
 routes:[
  {path:'/',name:'home',component:App},
  {path:'/projects/:group?',name:'project',component:App},
  {path:'/management',name:'management',component:App},
  {path:'/projects/:project/groups/:group?',name:'legacy-project',component:App},
  {path:'/projects/:project/management',name:'legacy-management',component:App},
  {path:'/:pathMatch(.*)*',redirect:'/'},
 ],
})
