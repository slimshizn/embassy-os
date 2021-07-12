import { Component } from '@angular/core'
import { Metrics } from 'src/app/services/api/api-types'
import { ApiService } from 'src/app/services/api/api.service'
import { pauseFor } from 'src/app/util/misc.util'

@Component({
  selector: 'server-metrics',
  templateUrl: './server-metrics.page.html',
  styleUrls: ['./server-metrics.page.scss'],
})
export class ServerMetricsPage {
  error = ''
  loading = true
  going = false
  metrics: Metrics = { }

  constructor (
    private readonly apiService: ApiService,
  ) { }

  ngOnInit () {
    this.loading = false
    this.startDaemon()
  }

  ngOnDestroy () {
    this.stopDaemon()
  }

  async startDaemon (): Promise<void> {
    this.going = true
    while (this.going) {
      await this.getMetrics()
      await pauseFor(250)
    }
  }

  stopDaemon () {
    this.going = false
  }

  async getMetrics (): Promise<void> {
    try {
      this.metrics = await this.apiService.getServerMetrics({ })
    } catch (e) {
      console.error(e)
      this.error = e.message
      this.stopDaemon()
    }
  }

  asIsOrder (a: any, b: any) {
    return 0
  }
}
