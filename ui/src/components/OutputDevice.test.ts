import { beforeEach, describe, expect, it } from 'vitest';
import { fireEvent, render, screen, waitFor } from '@testing-library/svelte';
import { MockBackend } from '$lib/mock';
import { shellSettings } from '$lib/shell.svelte';
import OutputDevice from './OutputDevice.svelte';

let mock: MockBackend;
beforeEach(async () => {
  mock = new MockBackend();
  shellSettings.devices = null;
  shellSettings.error = null;
  await shellSettings.init(mock);
});

describe('the output device choice', () => {
  it('follows the system default until a device is chosen, and a choice is saved and can be undone', async () => {
    render(OutputDevice);
    const select = (await screen.findByRole('combobox', { name: 'Output device' })) as HTMLSelectElement;
    expect(select.value).toBe('');
    expect([...select.options].map((o) => o.textContent)).toEqual(['Follow the system default', 'Speakers (Realtek Audio) (system default)', 'Headphones (USB DAC)']);
    await fireEvent.change(select, { target: { value: 'headphones' } });
    await waitFor(() => expect(mock.audioDeviceId).toBe('headphones'));
    expect(shellSettings.devices?.selected).toBe('headphones');
    await fireEvent.change(select, { target: { value: '' } });
    await waitFor(() => expect(mock.audioDeviceId).toBeNull());
  });

  it('shows a chosen device that is no longer connected, and does not offer it as a choice', async () => {
    mock.audioDeviceId = 'gone';
    render(OutputDevice);
    const select = (await screen.findByRole('combobox', { name: 'Output device' })) as HTMLSelectElement;
    const gone = [...select.options].find((o) => o.value === 'gone');
    expect(gone?.textContent).toContain('not connected');
    expect(gone?.disabled).toBe(true);
  });

  it('refuses an id the system did not list and puts the list back as it was', async () => {
    render(OutputDevice);
    await screen.findByRole('combobox', { name: 'Output device' });
    await shellSettings.chooseDevice('../../etc/passwd');
    expect(shellSettings.error).toBe('That output device is not connected.');
    expect(mock.audioDeviceId).toBeNull();
    expect(shellSettings.devices?.selected).toBeNull();
  });

  it('is not shown when the system lists no device', async () => {
    mock.audioDevices = [];
    render(OutputDevice);
    await waitFor(() => expect(shellSettings.devices).not.toBeNull());
    expect(screen.queryByRole('combobox', { name: 'Output device' })).toBeNull();
  });
});
