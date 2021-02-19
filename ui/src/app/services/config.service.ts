import { Injectable } from '@angular/core'

const { patchDb, maskAs, api, skipStartupAlerts } = require('../../../ui-config.json') as UiConfig

type UiConfig = {
  patchDb: {
    // If this is false (the default), poll will be used if in consulate only. If true it will be on regardless of env. This is useful in concert with api mocks.
    usePollOverride: boolean,
    poll: { cooldown: number /* in ms */ },
    websocket: { type: 'ws', url: string, version: number }
    // Wait this long (ms) before asking BE for a dump when out of order messages are received
    timeoutForMissingPatch: number
  }
  api: {
    mocks: boolean
    url: string
    version: string
    root: string
  }
  maskAs: 'tor' | 'lan' | 'none'
  skipStartupAlerts: boolean
}
@Injectable({
  providedIn: 'root',
})
export class ConfigService {
  origin = removePort(removeProtocol(window.origin))
  version = require('../../../package.json').version

  patchDb = patchDb
  api = api

  skipStartupAlerts  = skipStartupAlerts
  isConsulateIos     = window['platform'] === 'ios'
  isConsulateAndroid = window['platform'] === 'android'
  isConsulate = this.isConsulateIos || this.isConsulateAndroid

  isTor () : boolean {
    return (maskAs === 'tor') || this.origin.endsWith('.onion')
  }

  isLan () : boolean {
    return (maskAs === 'lan') || this.origin.endsWith('.local')
  }
}

function removeProtocol (str: string): string {
  if (str.startsWith('http://')) return str.slice(7)
  if (str.startsWith('https://')) return str.slice(8)
  return str
}

function removePort (str: string): string {
  return str.split(':')[0]
}
