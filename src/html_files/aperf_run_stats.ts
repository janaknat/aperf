let got_aperfstat_data = false;

let aperfstats_rules = {
    data_type: "aperf_run_stats",
    pretty_name: "Aperf Stats",
    rules: [
        {
            name: "Interval",
            all_run_rule: function* (opts): Generator<Finding, void, any> {
                //Older runs have 'interval'.
                if (!('interval_in_ms' in init_params_raw[opts.base_run]))
                    return;
                let base_interval = get_interval_data(opts.base_run);
                let interval_str = `${opts.base_run} has an interval: ${base_interval[0]}${base_interval[1]}. `;
                let interval_and_type = false;
                let interval_only = false;
                let other_runs = opts.runs.slice(1);
                for (let run of other_runs) {
                    let this_interval = get_interval_data(run);
                    interval_str = interval_str.concat(`${run} has an interval: ${this_interval[0]}${this_interval[1]}. `);
                    if (base_interval[0] != this_interval[0]) {
                        if (base_interval[1] != this_interval[1]) {
                            interval_and_type = true;
                        }
                        interval_only = true;
                    }
                }
                if (interval_and_type) {
                    yield new Finding(`Intervals don't match in time and units. ${interval_str}`, Status.NotGood);
                } else if (interval_only) {
                    yield new Finding(`Intervas don't match in time. ${interval_str}`, Status.NotGood);
                } else {
                    yield new Finding(`Intervals match in time and units. ${interval_str}`, Status.Good);
                }
            }
        }
    ]
}

function getAperfEntry(elem, key, run_data, run) {
    var value = JSON.parse(run_data);
    let collect = value.collect;
    let print = value.print;
    let x_collect = [];
    let y_collect = [];
    let x_print = [];
    let y_print = [];
    for (var i = 0; i < collect.length; i++) {
        x_collect.push(collect[i].time.TimeDiff);
        y_collect.push(collect[i].time_taken);
    }
    for (var i = 0; i < print.length; i++) {
        x_print.push(print[i].time.TimeDiff);
        y_print.push(print[i].time_taken);
    }
    var TESTER = elem;
    var aperfstat_collect_data: Partial<Plotly.PlotData> = {
        name: `${key}-collect`,
        x: x_collect,
        y: y_collect,
        type: 'scatter',
    };
    var aperfstat_print_data: Partial<Plotly.PlotData> = {
        name: `${key}-print`,
        x: x_print,
        y: y_print,
        type: 'scatter',
    };
    let limits = key_limits.get(key);
    var layout = {
        title: `${key}`,
        xaxis: {
            title: `Time (${get_x_axis_unit(run)})`,
        },
        yaxis: {
            title: 'Time (us)',
            range: [limits.low, limits.high],
        },
    }
    Plotly.newPlot(TESTER, [aperfstat_collect_data, aperfstat_print_data], layout, { frameMargins: 0 });
}

function getAperfEntries(run, container_id, keys, run_data) {
    for (let i = 0; i < all_run_keys.length; i++) {
        let value = all_run_keys[i];
	    var elem = document.createElement('div');
	    elem.id = `aperfstat-${run}-${value}`;
	    elem.style.float = "none";
        if (keys.length == 0) {
            elem.innerHTML = "No data collected";
            addElemToNode(container_id, elem);
            return;
        }
        addElemToNode(container_id, elem);
        emptyOrCallback(keys, false, getAperfEntry, elem, value, run_data, run);
    }
}

function aperfStat() {
    if (got_aperfstat_data) {
        return;
    }
    clear_and_create('aperfstat');
    form_graph_limits(aperf_run_stats_raw_data);
    for (let i = 0; i < aperf_run_stats_raw_data['runs'].length; i++) {
        let run_name = aperf_run_stats_raw_data['runs'][i]['name'];
        let elem_id = `${run_name}-aperfstat-per-data`;
        let this_run_data = aperf_run_stats_raw_data['runs'][i];
        getAperfEntries(run_name, elem_id, this_run_data['keys'], this_run_data['key_values']);
    }
    got_aperfstat_data = true;
}
