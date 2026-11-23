// %% @text name_try_performance_pads
{
    include('builtin/lib/performance-pads.js').open({
        id:'Performance pads', width:960, height:512,
        xy:{notes:[48, 51, 53, 55, 58, 60, 63, 65, 67, 70, 72, 75, 77, 79, 82, 84],
            controller:1, color:[45, 56, 76]},
        instrumentNote:60, instrumentChord:'bIIIM7',
        drums:{format:'CLAP', plugin:'sforzando', patch:'Free Sounds/Programs/Analog Industries/CR-909/main.sfz',
            notes:[36, 38, 42], percussion:[37, 39, 43, 45, 49, 50, 51]},
        colors:[
            [218, 89, 103], [230, 132, 83], [212, 166, 72], [163, 176, 72],
            [106, 180, 91], [66, 176, 128], [58, 174, 165], [71, 157, 197],
            [88, 120, 216], [123, 100, 219], [159, 94, 206], [192, 90, 168],
            [162, 71, 79], [166, 109, 63], [80, 133, 93], [115, 84, 164]
        ]
    });
}
