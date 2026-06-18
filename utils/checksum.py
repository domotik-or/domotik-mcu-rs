# frame = bytearray(b"SINST\t99999\t!")
frame = bytearray(b"EAST\t999999999\t!")
sum = (sum(frame[:-1]) + 0x20) & 0x7f
frame[-1] = sum
print(sum, " / " , frame, " / ", ' '.join('{:02x}'.format(x) for x in frame))
