import { type ReactNode, useEffect, useRef, useState } from 'react'
import { useNavigate } from 'react-router'

import { Button } from '@/components/ui/button'
import { startVoiceLevelMeter } from '@/lib/voice-level-meter'
import type { ConfigFieldSchema, HermesConfigRecord } from '@/types/hermes'

import { ConfigField } from './config-field'
import { enumOptionsFor, getNested, inferFieldSchema, setConfigField } from './helpers'

export function VoiceAdvancedOptions({ enabled, reveal, children }: { enabled: boolean; reveal: boolean; children: ReactNode }) {
  if (!enabled) {return <>{children}</>}

  return <details className="mt-6 rounded-lg border border-border bg-card p-4" open={reveal || undefined}>
    <summary className="cursor-pointer text-base font-semibold">Advanced voice options</summary>
    <p className="my-3 text-sm text-muted-foreground">Recording limits, live voice instructions and direct connection settings.</p>
    {children}
  </details>
}

function useMicrophoneCheck() {
  const [status, setStatus] = useState('Not tested')
  const [testing, setTesting] = useState(false)
  const [level, setLevel] = useState(0)
  const generation = useRef(0)
  const release = useRef<() => void>(() => undefined)

  const cancel = () => { generation.current++; release.current(); setTesting(false); setLevel(0); setStatus('Not tested') }

  useEffect(() => {
    const hide = () => { if (document.hidden) {cancel()} }
    document.addEventListener('visibilitychange', hide)

    return () => { document.removeEventListener('visibilitychange', hide); generation.current++; release.current() }
  }, [])

  const start = async () => {
    if (!navigator.mediaDevices?.getUserMedia) { setStatus('Microphone capture is unavailable in this application environment.');

 return }

    const request = ++generation.current
    release.current()
    setTesting(true); setStatus('Waiting for microphone access…'); setLevel(0)
    let stream: MediaStream | undefined
    let context: AudioContext | undefined
    let meter: ReturnType<typeof startVoiceLevelMeter> | undefined
    let timer: ReturnType<typeof setTimeout> | undefined

    const cleanup = () => { clearTimeout(timer); meter?.stop(); stream?.getTracks().forEach(track => track.stop()); void context?.close().catch(() => undefined) }
    release.current = cleanup

    try {
      stream = await navigator.mediaDevices.getUserMedia({ audio: { echoCancellation: true, noiseSuppression: true }, video: false })

      if (generation.current !== request) { stream.getTracks().forEach(track => track.stop());

 return }

      const Constructor = window.AudioContext || (window as unknown as { webkitAudioContext?: typeof AudioContext }).webkitAudioContext

      if (!Constructor) {throw new Error('Audio monitoring is unavailable. Microphone access alone does not verify audio.')}
      context = new Constructor()
      let heard = false

      const fail = () => {
        if (generation.current !== request) {return}
        generation.current++; cleanup(); setTesting(false); setLevel(0); setStatus('Microphone test failed. Check your audio input device.')
      }

      meter = startVoiceLevelMeter(context, context.createMediaStreamSource(stream), (value, _time, current) => {
        if (generation.current !== request) {return}

        if (value > 0.02) {heard = true}

        if (current) {setLevel(value)}
      }, fail)
      const ready = await meter.ready

      if (!ready || generation.current !== request) {return}
      setStatus('Listening… say a few words.')
      timer = setTimeout(() => {
        if (generation.current !== request) {return}
        generation.current++; cleanup(); setTesting(false); setLevel(0)
        setStatus(heard ? 'Audio detected' : 'No audio detected. Check the microphone and try again.')
      }, 5000)
    } catch (error) {
      cleanup()

      if (generation.current !== request) {return}
      generation.current++; setTesting(false); setLevel(0)
      setStatus(error instanceof DOMException && error.name === 'NotAllowedError'
        ? 'Microphone access was blocked. Check microphone permission in system settings.'
        : error instanceof Error ? error.message : 'Could not test the microphone.')
    }
  }

  return { status, testing, level, start, cancel }
}

/** Common voice setup; existing provider editors and advanced fields remain authoritative. */
export function VoiceSetup({ config, schema, onChange }: {
  config: HermesConfigRecord; schema: Record<string, ConfigFieldSchema>; onChange: (config: HermesConfigRecord) => void
}) {
  const navigate = useNavigate()
  const microphone = useMicrophoneCheck()
  const mode = getNested(config, 'voice.voice_chat_mode')
  const open = (page: string) => navigate(`/settings?tab=config%3Avoice&page=${page}`)

  const provider = (key: 'stt.provider' | 'tts.provider') => <ConfigField enumOptions={enumOptionsFor(key, getNested(config, key), config)}
    onChange={value => onChange(setConfigField(config, key, value))} schema={schema[key] ?? inferFieldSchema(getNested(config, key))}
    schemaKey={key}
    value={getNested(config, key)} />

  return <div className="grid gap-6">
    <section aria-label="Conversation mode" className="border-y border-border py-5">
      <h3 className="mb-3 text-base font-semibold">Conversation mode</h3>
      <div aria-label="Conversation mode" className="grid gap-3 lg:grid-cols-2" role="radiogroup">
        {[
          ['chained', 'Standard', 'Speech to text, Hermes, then a spoken reply.'],
          ['gpt-live', 'Live conversation', 'A lower-latency, back-and-forth voice conversation.']
        ].map(([value, label, description]) => <label className={`flex cursor-pointer items-start gap-3 rounded-lg border p-4 ${mode === value ? 'border-primary bg-primary/10' : 'border-border bg-card'}`} key={value}>
          <input checked={mode === value} className="mt-1 accent-(--primary)" name="voice-conversation-mode" onChange={() => onChange(setConfigField(config, 'voice.voice_chat_mode', value))} type="radio" value={value} />
          <span><span className="block text-sm font-semibold">{label}</span><span className="mt-1 block text-sm text-muted-foreground">{description}</span></span>
        </label>)}
      </div>
      {mode != null && !['chained', 'gpt-live'].includes(String(mode)) && <p className="mt-3 text-sm text-muted-foreground">Current custom mode: {String(mode)}. Select a mode to change it.</p>}
    </section>
    <section aria-label="Get voice ready">
      <h3 className="text-xl font-semibold">Get voice ready</h3>
      <p className="mt-2 text-sm text-muted-foreground">Set up your microphone and speech providers, then try voice in Chat.</p>
      <ol className="my-5 flex flex-wrap gap-x-8 gap-y-3 border-b border-border pb-5 text-sm">
        <li><span className="mr-2 inline-flex size-7 items-center justify-center rounded-full bg-primary text-primary-foreground">1</span>Microphone</li>
        <li><span className="mr-2 inline-flex size-7 items-center justify-center rounded-full border border-border">2</span>Speech providers</li>
        <li><span className="mr-2 inline-flex size-7 items-center justify-center rounded-full border border-border">3</span>Try voice in Chat</li>
      </ol>
      <div className="flex flex-wrap items-center justify-between gap-4 border-b border-border pb-5">
        <div><h4 className="font-semibold">Microphone</h4><p className="mt-1 text-sm text-muted-foreground" role="status">{microphone.status}</p>
          {microphone.testing && <progress aria-label="Microphone input level" className="mt-2 h-2 w-48 accent-(--primary)" max={1} value={microphone.level} />}
        </div>
        <Button onClick={() => microphone.testing ? microphone.cancel() : void microphone.start()} variant="outline">{microphone.testing ? 'Cancel microphone test' : 'Test microphone…'}</Button>
      </div>
      <p className="mt-2 text-xs text-muted-foreground">The microphone test stays on this device and stops after five seconds.</p>
      <div className="mt-3 border-b border-border">{provider('stt.provider')}
        <Button onClick={() => open('transcription')} size="inline" variant="text">Configure speech recognition</Button>
      </div>
      <div className="border-b border-border py-3">{provider('tts.provider')}
        <Button onClick={() => open('speech')} size="inline" variant="text">Choose a voice and configure spoken replies</Button>
      </div>
      <p className="mt-4 rounded-lg border border-border bg-card p-3 text-sm text-muted-foreground">Provider selection does not verify credentials or a working voice connection. Configure the providers before starting a conversation.</p>
      <div className="mt-4 flex flex-wrap gap-4">
        <Button onClick={() => navigate('/')} variant="outline">Open Chat to try voice</Button>
        <Button onClick={() => navigate('/settings?tab=keys')} size="inline" variant="text">Manage API keys</Button>
      </div>
    </section>
  </div>
}
