import { beforeEach, describe, expect, it } from 'vitest';
import { fireEvent, render, screen, within } from '@testing-library/svelte';
import EqSection from './EqSection.svelte';
import { eq } from '$lib/eq.svelte';
import { FLAT_EQ } from '$lib/eq';
import { MockBackend } from '$lib/mock';

let mock: MockBackend;

beforeEach(async () => {
  mock = new MockBackend();
  eq.settings = { ...FLAT_EQ, gains: [...FLAT_EQ.gains] };
  await eq.init(mock);
});

const slider = (name: string) => screen.getByRole('slider', { name }) as HTMLInputElement;

describe('EqSection', () => {
  it('shows ten band sliders and a preamp, all at 0 dB, with the equalizer off', () => {
    render(EqSection);
    for (const name of ['31 Hz', '62 Hz', '125 Hz', '250 Hz', '500 Hz', '1 kHz', '2 kHz', '4 kHz', '8 kHz', '16 kHz', 'Preamp']) {
      expect(slider(name).value, name).toBe('0');
    }
    expect((screen.getByRole('checkbox', { name: 'Equalizer' }) as HTMLInputElement).checked).toBe(false);
    expect(screen.queryByRole('button', { name: 'Reset to flat' })).toBeNull();
  });
  it('turns on and off, and saves each switch', async () => {
    render(EqSection);
    await fireEvent.click(screen.getByRole('checkbox', { name: 'Equalizer' }));
    expect(mock.eq.enabled).toBe(true);
    expect(mock.eqCalls.at(-1)!.save).toBe(true);
    await fireEvent.click(screen.getByRole('checkbox', { name: 'Equalizer' }));
    expect(mock.eq.enabled).toBe(false);
  });
  it('sends every step of a drag to the engine but saves only when the slider is let go', async () => {
    render(EqSection);
    const s = slider('1 kHz');
    await fireEvent.input(s, { target: { value: '30' } });
    await fireEvent.input(s, { target: { value: '45' } });
    expect(mock.eqCalls.map((c) => c.save)).toEqual([false, false]);
    expect(mock.eq.gains[5]).toBe(45);
    await fireEvent.change(s, { target: { value: '45' } });
    expect(mock.eqCalls.at(-1)).toMatchObject({ save: true });
    expect(mock.eq.gains[5]).toBe(45);
    expect(s.getAttribute('aria-valuetext')).toBe('+4.5 dB');
  });
  it('moves the preamp the same way', async () => {
    render(EqSection);
    await fireEvent.input(slider('Preamp'), { target: { value: '-60' } });
    expect(mock.eq.preamp).toBe(-60);
    expect(mock.eqCalls.at(-1)!.save).toBe(false);
    expect(slider('Preamp').getAttribute('aria-valuetext')).toBe('\u22126 dB');
  });
  it('applies a preset, marks it pressed, and calls it custom after a band moves', async () => {
    render(EqSection);
    const presets = screen.getByRole('group', { name: 'Preset' });
    await fireEvent.click(within(presets).getByRole('button', { name: 'Bass boost' }));
    expect(mock.eq.gains.slice(0, 4)).toEqual([60, 50, 40, 20]);
    expect(mock.eqCalls.at(-1)!.save).toBe(true);
    expect(within(presets).getByRole('button', { name: 'Bass boost' }).getAttribute('aria-pressed')).toBe('true');
    await fireEvent.input(slider('8 kHz'), { target: { value: '10' } });
    for (const b of within(presets).getAllByRole('button')) expect(b.getAttribute('aria-pressed')).toBe('false');
  });
  it('resets to flat but leaves the switch alone', async () => {
    render(EqSection);
    await fireEvent.click(screen.getByRole('checkbox', { name: 'Equalizer' }));
    await fireEvent.click(within(screen.getByRole('group', { name: 'Preset' })).getByRole('button', { name: 'Treble' }));
    await fireEvent.input(slider('Preamp'), { target: { value: '20' } });
    await fireEvent.click(screen.getByRole('button', { name: 'Reset to flat' }));
    expect(mock.eq.gains).toEqual(new Array(10).fill(0));
    expect(mock.eq.preamp).toBe(0);
    expect(mock.eq.enabled).toBe(true);
    expect(screen.queryByRole('button', { name: 'Reset to flat' })).toBeNull();
  });
  it('keeps what it shows inside the limits and says so when the engine refuses', async () => {
    render(EqSection);
    await eq.update({ preamp: 9999, gains: [500, ...new Array(9).fill(0)] }, false);
    expect(eq.settings.preamp).toBe(120);
    expect(eq.settings.gains[0]).toBe(120);
    mock.eqSet = async () => {
      throw 'engine is not running';
    };
    await fireEvent.input(slider('250 Hz'), { target: { value: '5' } });
    expect(await screen.findByRole('alert')).toBeTruthy();
    expect(screen.getByRole('alert').textContent).toContain('engine is not running');
  });
});
