#!/usr/bin/perl
# 隔离环境透明代理，只记录实际插件的回调方法名
use strict;
use warnings;
pipe(my $child_in, my $input) or die "pipe";
pipe(my $output, my $child_out) or die "pipe";
my $pid = fork();
defined $pid or die "fork";
if (!$pid) {
    close $input; close $output;
    open STDIN, '<&', $child_in or die "stdin";
    open STDOUT, '>&', $child_out or die "stdout";
    exec '/validation/real-plugin';
    die "exec";
}
close $child_in; close $child_out;
my $pump = fork();
defined $pump or die "fork";
sub write_all {
    my ($fh, $data) = @_;
    my $at = 0;
    while ($at < length $data) {
        my $n = syswrite($fh, $data, length($data)-$at, $at);
        defined $n && $n > 0 or die "write";
        $at += $n;
    }
}
if (!$pump) {
    close $output;
    while (sysread(STDIN, my $bytes, 65536)) { write_all($input, $bytes); }
    close $input;
    exit;
}
close $input;
sub exact {
    my ($n) = @_;
    my $data = '';
    while (length($data)<$n) {
        my $read = sysread($output, my $bytes, $n-length($data));
        return undef unless $read;
        $data .= $bytes;
    }
    return $data;
}
open my $log, '>>', '/validation/callback-methods.log' or die "log";
select((select($log), $|=1)[0]);
while (defined(my $header=exact(12))) {
    my ($meta, $payload)=unpack('NQ>', $header);
    $meta <= 65536 && $payload <= 8388608 or die "frame size";
    my $metadata=exact($meta);
    my $body=exact($payload);
    defined $metadata && defined $body or last;
    if ($metadata =~ /"type"\s*:\s*"callback"/ && $metadata =~ /"method"\s*:\s*"([^"]+)"/) {
        print $log "$1\n";
    }
    write_all(\*STDOUT, $header.$metadata.$body);
}
waitpid($pid, 0);
kill 'TERM', $pump;
