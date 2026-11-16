signal = [0, 1, 0, 0]
smoothed = movingavg(signal, 2)
spectrum = fft(signal)
padded_spectrum = fft(append([1, 2, 3], [0]))
wave = [1, 2, 3, 4, 5, 6, 7, 8]
default_spectrum = fft(wave)
setprecision(50)
wide_spectrum = fft(wave)
