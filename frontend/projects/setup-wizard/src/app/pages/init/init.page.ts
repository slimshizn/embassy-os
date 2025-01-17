import { Component } from '@angular/core'
import { interval, Subscription } from 'rxjs'
import { finalize, take, tap } from 'rxjs/operators'
import { ApiService } from 'src/app/services/api/api.service'
import { StateService } from 'src/app/services/state.service'

@Component({
  selector: 'app-init',
  templateUrl: 'init.page.html',
  styleUrls: ['init.page.scss'],
})
export class InitPage {
  progress = 0
  sub: Subscription

  constructor (
    private readonly apiService: ApiService,
    public readonly stateService: StateService,
  ) { }

  ngOnInit () {
    // call setup.complete to tear down embassy.local and spin up embassy-[id].local
    this.apiService.setupComplete()

    this.sub = interval(130)
    .pipe(
      take(101),
      tap(num => {
        this.progress = num
      }),
      finalize(() => {
        setTimeout(() => {
          this.stateService.embassyLoaded = true
          this.download()
        }, 500)
      }),
    ).subscribe()
  }

  ngOnDestroy () {
    if (this.sub) this.sub.unsubscribe()
  }

  download () {
    document.getElementById('tor-addr').innerHTML = this.stateService.torAddress
    document.getElementById('lan-addr').innerHTML = this.stateService.lanAddress
    document.getElementById('cert').setAttribute('href', 'data:application/x-x509-ca-cert;base64,' + encodeURIComponent(this.stateService.cert))
    let html = document.getElementById('downloadable').innerHTML
    const filename = 'embassy-info.html'

    const elem = document.createElement('a')
    elem.setAttribute('href', 'data:text/plain;charset=utf-8,' + encodeURIComponent(html))
    elem.setAttribute('download', filename)
    elem.style.display = 'none'

    document.body.appendChild(elem)
    elem.click()
    document.body.removeChild(elem)
  }
}

