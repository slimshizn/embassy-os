import { Injectable } from '@angular/core'
import { Http } from 'patch-db-client'
import { DataModel } from '../models/patch-db/data-model'

const { patchDb, maskAs, useMocks, skipStartupAlerts } = require('../../../ui-config.json') as UiConfig

type UiConfig = {
  patchDb: {
    http  : { type: 'mock' } | { type: 'live', url: string }
    source:
        { type: 'poll', cooldown: number  /* in ms */ } 
      | { type: 'ws', url: string, version: number }
  }

  useMocks: boolean //@TODO 0.3.0: Deprecated, remove for 0.3.0
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

  api = {
    useMocks: useMocks,
    url: '/api',
    version: '/v0',
    root: '', // empty will default to same origin
  }

  skipStartupAlerts  = skipStartupAlerts
  isConsulateIos     = window['platform'] === 'ios'
  isConsulateAndroid = window['platform'] === 'android'

  isTor () : boolean {
    return (this.api.useMocks && maskAs === 'tor') || this.origin.endsWith('.onion')
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
