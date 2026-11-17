m = sys_closed(2, 9/100, [sys_station("cpu", 1/100, 1)])
sys_mva(m).throughput

net = sys_open(8, [sys_station("api", 1/20, 1), sys_station("db", 1/50, 2)])
sys_analyze(net).response_time
sys_bottlenecks(net).throughput_bound
