import { NgModule } from '@angular/core'
import { CommonModule } from '@angular/common'
import { IonicModule } from '@ionic/angular'
import { RouterModule, Routes } from '@angular/router'
import { SSHKeysPage } from './ssh-keys.page'
import { PwaBackComponentModule } from 'src/app/components/pwa-back-button/pwa-back.component.module'
import { SharingModule } from 'src/app/modules/sharing.module'
import { TextSpinnerComponentModule } from 'src/app/components/text-spinner/text-spinner.component.module'

const routes: Routes = [
  {
    path: '',
    component: SSHKeysPage,
  },
]

@NgModule({
  imports: [
    CommonModule,
    IonicModule,
    RouterModule.forChild(routes),
    PwaBackComponentModule,
    SharingModule,
    TextSpinnerComponentModule,
  ],
  declarations: [SSHKeysPage],
})
export class SSHKeysPageModule { }