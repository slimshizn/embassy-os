import { Injectable } from '@angular/core'

const { patchDb, maskAs, api, skipStartupAlerts } = require('../../../ui-config.json') as UiConfig

// patchDb.type === 'ws' and api.mocks === true presently does not work. Mocks are intended to be used with poll.
type UiConfig = {
  patchDb: { type: 'poll', cooldown: number  /* in ms */ } | { type: 'ws', url: string, version: number }
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
